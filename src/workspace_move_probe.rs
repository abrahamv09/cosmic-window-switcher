// SPDX-License-Identifier: GPL-3.0-only

use anyhow::{Context, Result, bail};
use cosmic_client_toolkit::{
    GlobalData,
    toplevel_management::{ToplevelManagerHandler, ToplevelManagerState},
    workspace::{WorkspaceHandler, WorkspaceState},
};
use cosmic_protocols::{
    toplevel_info::v1::client::{zcosmic_toplevel_handle_v1, zcosmic_toplevel_info_v1},
    toplevel_management::v1::client::zcosmic_toplevel_manager_v1,
};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use smithay_client_toolkit::{
    delegate_output, delegate_registry,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
};
use std::time::{Duration, Instant};
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum,
    globals::{GlobalList, registry_queue_init},
    protocol::wl_output,
};
use wayland_protocols::ext::{
    foreign_toplevel_list::v1::client::{
        ext_foreign_toplevel_handle_v1, ext_foreign_toplevel_list_v1,
    },
    workspace::v1::client::ext_workspace_handle_v1,
};

use cosmic_window_switcher::{
    ManagementCapabilityContract, WindowId, WorkspaceId, WorkspaceMoveCapabilities,
    WorkspaceMoveVerification,
};

const TOPLEVEL_INFO_INTERFACE: &str = "zcosmic_toplevel_info_v1";
const TOPLEVEL_MANAGER_INTERFACE: &str = "zcosmic_toplevel_manager_v1";
const EXT_WORKSPACE_INTERFACE: &str = "ext_workspace_manager_v1";
const REQUIRED_TOPLEVEL_INFO_VERSION: u32 = 3;
const REQUIRED_TOPLEVEL_MANAGER_VERSION: u32 = 4;
const REQUIRED_EXT_WORKSPACE_VERSION: u32 = 1;
const MOVE_CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone, Copy)]
pub enum Mode<'a> {
    Inventory,
    Move(MoveArguments<'a>),
}

#[derive(Clone, Copy)]
pub struct MoveArguments<'a> {
    window_id: &'a str,
    workspace_id: &'a str,
    output_name: Option<&'a str>,
}

impl<'a> Mode<'a> {
    pub fn from_options(
        window_id: Option<&'a str>,
        workspace_id: Option<&'a str>,
        output_name: Option<&'a str>,
    ) -> Result<Self> {
        match (window_id, workspace_id, output_name) {
            (None, None, None) => Ok(Self::Inventory),
            (Some(window_id), Some(workspace_id), output_name) => Ok(Self::Move(MoveArguments {
                window_id,
                workspace_id,
                output_name,
            })),
            (None, None, Some(_)) => bail!("--output requires --window and --workspace"),
            _ => bail!("--window and --workspace must be supplied together"),
        }
    }
}

pub fn run(mode: Mode<'_>) -> Result<()> {
    crate::cosmic_session::verify("workspace-move probe")?;

    let connection = Connection::connect_to_env().context("connect to the Wayland compositor")?;
    let (globals, mut event_queue) =
        registry_queue_init(&connection).context("read Wayland globals")?;
    let queue_handle = event_queue.handle();
    let mut probe = create_probe(&globals, &queue_handle)?;
    receive_initial_snapshot(&mut probe, &mut event_queue)?;
    probe.print_inventory();

    match mode {
        Mode::Inventory => {
            println!(
                "Inventory only; pass --window <id> --workspace <id> [--output <name>] to verify one move."
            );
            Ok(())
        }
        Mode::Move(arguments) => verify_move(&mut probe, &mut event_queue, &arguments),
    }
}

fn create_probe(
    globals: &GlobalList,
    queue_handle: &QueueHandle<WorkspaceMoveProbe>,
) -> Result<WorkspaceMoveProbe> {
    let toplevel_info_version = require_global_version(
        globals,
        TOPLEVEL_INFO_INTERFACE,
        REQUIRED_TOPLEVEL_INFO_VERSION,
    )?;
    let manager_version = require_global_version(
        globals,
        TOPLEVEL_MANAGER_INTERFACE,
        REQUIRED_TOPLEVEL_MANAGER_VERSION,
    )?;
    let workspace_version = require_global_version(
        globals,
        EXT_WORKSPACE_INTERFACE,
        REQUIRED_EXT_WORKSPACE_VERSION,
    )?;

    let registry_state = RegistryState::new(globals);
    let _foreign_toplevel_list = registry_state
        .bind_one::<ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1, _, _>(
            queue_handle,
            1..=1,
            GlobalData,
        )
        .context("bind foreign Window discovery")?;
    let cosmic_toplevel_info = registry_state
        .bind_one::<zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1, _, _>(
            queue_handle,
            3..=3,
            GlobalData,
        )
        .context("bind COSMIC Window workspace state")?;
    Ok(WorkspaceMoveProbe {
        output_state: OutputState::new(globals, queue_handle),
        cosmic_toplevel_info,
        windows: Vec::new(),
        workspace_state: WorkspaceState::new(&registry_state, queue_handle),
        toplevel_manager_state: ToplevelManagerState::try_new(&registry_state, queue_handle)
            .context("bind COSMIC Window management")?,
        registry_state,
        advertised_toplevel_info_version: toplevel_info_version,
        advertised_manager_version: manager_version,
        advertised_workspace_version: workspace_version,
        management_capabilities: None,
        workspace_snapshot_received: false,
        toplevel_snapshot_generation: 0,
    })
}

fn receive_initial_snapshot(
    probe: &mut WorkspaceMoveProbe,
    event_queue: &mut EventQueue<WorkspaceMoveProbe>,
) -> Result<()> {
    for round in 0..8 {
        event_queue
            .roundtrip(probe)
            .context("receive COSMIC Window and workspace state")?;
        if round >= 2 && probe.initial_snapshot_ready() {
            break;
        }
    }
    if !probe.initial_snapshot_ready() {
        bail!(
            "COSMIC did not provide a complete Window, workspace, and management capability snapshot"
        );
    }
    Ok(())
}

fn verify_move(
    probe: &mut WorkspaceMoveProbe,
    event_queue: &mut EventQueue<WorkspaceMoveProbe>,
    arguments: &MoveArguments<'_>,
) -> Result<()> {
    let contract = ManagementCapabilityContract::new(
        probe.advertised_manager_version,
        probe
            .management_capabilities
            .as_ref()
            .expect("the initial capability snapshot is ready")
            .workspace_move,
    );
    contract
        .verify_workspace_move()
        .map_err(|failure| anyhow::anyhow!("failed workspace-move capability: {failure:?}"))?;
    if probe.toplevel_snapshot_generation == 0 {
        bail!(
            "failed workspace-move capability: COSMIC toplevel-info v3 did not provide an atomic Window membership snapshot"
        );
    }
    let selected = probe.select_move(arguments)?;
    let mut verification = WorkspaceMoveVerification::new(
        WindowId::from(arguments.window_id),
        selected.original_membership.clone(),
        selected.target.clone(),
    )
    .map_err(|failure| anyhow::anyhow!("workspace move cannot start: {failure:?}"))?;
    let request = verification
        .request(&contract)
        .map_err(|failure| anyhow::anyhow!("failed workspace-move capability: {failure:?}"))?;

    let snapshot_generation = probe.toplevel_snapshot_generation;
    probe.toplevel_manager_state.manager.move_to_ext_workspace(
        &selected.window,
        &selected.workspace,
        &selected.output,
    );
    println!(
        "Issued move_to_ext_workspace for Window id={} exactly once.",
        request.window
    );

    if let Err(rejection) = wait_for_membership_snapshot(
        probe,
        event_queue,
        snapshot_generation,
        &selected.foreign_window,
        &selected.workspace,
        MOVE_CONFIRMATION_TIMEOUT,
    ) {
        return report_rejected_request(
            arguments.window_id,
            &selected.original_membership,
            &rejection,
        );
    }

    let resulting_membership = probe.workspace_membership(&selected.foreign_window)?;
    let verified = verification.verify(resulting_membership).map_err(|failure| {
        anyhow::anyhow!(
            "failed workspace-move capability: {failure:?}; the compositor advertised capability 8 but did not confirm the requested target"
        )
    })?;
    println!(
        "Verified Window id={} moved exactly once; original workspace membership={:?}, resulting workspace membership={:?}.",
        verified.window, verified.original_membership, verified.resulting_membership
    );
    Ok(())
}

fn wait_for_membership_snapshot(
    probe: &mut WorkspaceMoveProbe,
    event_queue: &mut EventQueue<WorkspaceMoveProbe>,
    initial_generation: u64,
    foreign_window: &ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
    target_workspace: &ext_workspace_handle_v1::ExtWorkspaceHandleV1,
    timeout: Duration,
) -> Result<()> {
    event_queue
        .flush()
        .context("flush the workspace-move request")?;
    let deadline = Instant::now() + timeout;
    loop {
        event_queue
            .dispatch_pending(probe)
            .context("dispatch resulting workspace membership")?;
        if probe.toplevel_snapshot_generation > initial_generation
            && probe.windows.iter().any(|window| {
                window.foreign_toplevel == *foreign_window
                    && window.committed_workspaces.contains(target_workspace)
            })
        {
            break;
        }
        let Some(read_guard) = event_queue.prepare_read() else {
            continue;
        };
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            drop(read_guard);
            break;
        }
        let timeout =
            Timespec::try_from(remaining).context("represent move confirmation timeout")?;
        let mut poll_fds = [PollFd::from_borrowed_fd(
            read_guard.connection_fd(),
            PollFlags::IN,
        )];
        if poll(&mut poll_fds, Some(&timeout)).context("wait for workspace membership events")? == 0
        {
            drop(read_guard);
            break;
        }
        read_guard
            .read()
            .context("read resulting workspace membership")?;
    }
    event_queue
        .dispatch_pending(probe)
        .context("dispatch final workspace membership")?;
    Ok(())
}

fn report_rejected_request(
    window_id: &str,
    original_membership: &[WorkspaceId],
    rejection: &anyhow::Error,
) -> Result<()> {
    let resulting_membership = query_workspace_membership(window_id).with_context(|| {
        format!(
            "failed workspace-move capability: compositor rejected the request ({rejection:#}); could not verify that Window {window_id} remained in its original workspace"
        )
    })?;
    if resulting_membership == original_membership {
        bail!(
            "failed workspace-move capability: compositor rejected the request ({rejection:#}); Window {window_id} remained in original workspace membership={original_membership:?}"
        );
    }
    bail!(
        "failed workspace-move capability: compositor rejected the request ({rejection:#}); Window {window_id} unexpectedly changed to workspace membership={resulting_membership:?}"
    )
}

fn query_workspace_membership(window_id: &str) -> Result<Vec<WorkspaceId>> {
    let connection = Connection::connect_to_env().context("reconnect to the Wayland compositor")?;
    let (globals, mut event_queue) =
        registry_queue_init(&connection).context("read Wayland globals after rejection")?;
    let mut probe = create_probe(&globals, &event_queue.handle())?;
    receive_initial_snapshot(&mut probe, &mut event_queue)?;
    if probe.toplevel_snapshot_generation == 0 {
        bail!("COSMIC toplevel-info v3 did not provide an atomic Window membership snapshot");
    }
    let window = probe
        .windows
        .iter()
        .find(|window| window.identifier == window_id)
        .with_context(|| format!("Window {window_id} is no longer discoverable"))?;
    probe.workspace_membership(&window.foreign_toplevel)
}

fn require_global_version(
    globals: &GlobalList,
    interface: &str,
    required_version: u32,
) -> Result<u32> {
    let version = globals
        .contents()
        .with_list(|list| {
            list.iter()
                .find(|global| global.interface == interface)
                .map(|global| global.version)
        })
        .with_context(|| format!("the compositor does not advertise {interface}"))?;
    if version < required_version {
        bail!(
            "the compositor advertises {interface} version {version}, but version {required_version} is required"
        );
    }
    Ok(version)
}

struct SelectedMove {
    foreign_window: ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
    window: zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1,
    workspace: ext_workspace_handle_v1::ExtWorkspaceHandleV1,
    output: wl_output::WlOutput,
    target: WorkspaceId,
    original_membership: Vec<WorkspaceId>,
}

struct WorkspaceWindow {
    foreign_toplevel: ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
    cosmic_toplevel: zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1,
    identifier: String,
    app_id: String,
    outputs: std::collections::HashSet<wl_output::WlOutput>,
    workspaces: std::collections::HashSet<ext_workspace_handle_v1::ExtWorkspaceHandleV1>,
    committed_workspaces: std::collections::HashSet<ext_workspace_handle_v1::ExtWorkspaceHandleV1>,
    metadata_complete: bool,
}

struct WorkspaceMoveProbe {
    registry_state: RegistryState,
    output_state: OutputState,
    cosmic_toplevel_info: zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1,
    windows: Vec<WorkspaceWindow>,
    workspace_state: WorkspaceState,
    toplevel_manager_state: ToplevelManagerState,
    advertised_toplevel_info_version: u32,
    advertised_manager_version: u32,
    advertised_workspace_version: u32,
    management_capabilities: Option<AdvertisedManagementCapabilities>,
    workspace_snapshot_received: bool,
    toplevel_snapshot_generation: u64,
}

struct AdvertisedManagementCapabilities {
    raw: Vec<u32>,
    workspace_move: WorkspaceMoveCapabilities,
}

impl WorkspaceMoveProbe {
    fn initial_snapshot_ready(&self) -> bool {
        self.workspace_snapshot_received
            && self.management_capabilities.is_some()
            && self.windows.iter().all(|window| window.metadata_complete)
    }

    fn print_inventory(&self) {
        let capabilities = self
            .management_capabilities
            .as_ref()
            .expect("the initial capability snapshot is ready");
        println!(
            "Advertised protocols: {TOPLEVEL_MANAGER_INTERFACE}=v{} (bound v{}), {TOPLEVEL_INFO_INTERFACE}=v{}, {EXT_WORKSPACE_INTERFACE}=v{}.",
            self.advertised_manager_version,
            self.toplevel_manager_state.manager.version(),
            self.advertised_toplevel_info_version,
            self.advertised_workspace_version,
        );
        println!(
            "Advertised Window management capabilities={:?}.",
            capabilities.raw
        );

        let groups = self.workspace_state.workspace_groups().collect::<Vec<_>>();
        let topology = if groups.len() == 1 && groups[0].outputs.len() > 1 {
            "spanning workspace group"
        } else if groups.len() > 1 {
            "separate-display workspace groups"
        } else {
            "single-output workspace group"
        };
        println!("Workspace topology: {topology}; groups={}.", groups.len());
        for (index, group) in groups.iter().enumerate() {
            let outputs = group
                .outputs
                .iter()
                .map(|output| self.output_label(output))
                .collect::<Vec<_>>();
            println!("Workspace group {} outputs={outputs:?}.", index + 1);
            let mut workspaces = self
                .workspace_state
                .workspaces()
                .filter(|workspace| group.workspaces.contains(&workspace.handle))
                .collect::<Vec<_>>();
            workspaces.sort_by(|left, right| left.coordinates.cmp(&right.coordinates));
            for workspace in workspaces {
                let selector = self.workspace_selector(&workspace.handle);
                println!(
                    "  Workspace selector={selector:?} id={} name={:?} coordinates={:?} state={:?}.",
                    workspace.id.as_deref().unwrap_or("<not-advertised>"),
                    workspace.name,
                    workspace.coordinates,
                    workspace.state,
                );
            }
        }

        for window in self
            .windows
            .iter()
            .filter(|window| window.metadata_complete)
        {
            let memberships = self.membership_ids(&window.committed_workspaces);
            println!(
                "Window id={} app_id={:?} workspace membership={memberships:?}.",
                window.identifier, window.app_id
            );
        }
        if self.toplevel_snapshot_generation == 0 {
            println!(
                "COSMIC toplevel-info v3 has not emitted an atomic done event; empty Window membership is unverified."
            );
        }
    }

    fn select_move(&self, arguments: &MoveArguments<'_>) -> Result<SelectedMove> {
        let MoveArguments {
            window_id: requested_window_id,
            workspace_id: requested_workspace_id,
            output_name: requested_output_name,
        } = *arguments;
        let window = self
            .windows
            .iter()
            .find(|window| window.identifier == requested_window_id)
            .with_context(|| format!("no Window has id={requested_window_id}"))?;
        let mut matching_workspaces = self
            .workspace_state
            .workspaces()
            .filter(|workspace| {
                self.workspace_selector(&workspace.handle) == requested_workspace_id
                    || workspace.id.as_deref() == Some(requested_workspace_id)
                    || workspace.name == requested_workspace_id
            })
            .collect::<Vec<_>>();
        if let Some(output_name) = requested_output_name {
            matching_workspaces.retain(|workspace| {
                self.workspace_state.workspace_groups().any(|group| {
                    group.workspaces.contains(&workspace.handle)
                        && group
                            .outputs
                            .iter()
                            .any(|output| self.output_name(output).as_deref() == Some(output_name))
                })
            });
        }
        let workspace = match matching_workspaces.as_slice() {
            [] => bail!("no workspace matches selector={requested_workspace_id:?}"),
            [workspace] => *workspace,
            _ => bail!(
                "workspace selector={requested_workspace_id:?} is ambiguous; use the exact selector printed by inventory or pass --output"
            ),
        };
        let group = self
            .workspace_state
            .workspace_groups()
            .find(|group| group.workspaces.contains(&workspace.handle))
            .with_context(|| {
                format!("workspace {requested_workspace_id} does not belong to an output group")
            })?;
        let output = self.select_output(
            group.outputs.as_slice(),
            &window.outputs,
            requested_output_name,
        )?;
        let target = WorkspaceId::from(self.workspace_selector(&workspace.handle));

        Ok(SelectedMove {
            foreign_window: window.foreign_toplevel.clone(),
            window: window.cosmic_toplevel.clone(),
            workspace: workspace.handle.clone(),
            output,
            target,
            original_membership: self.workspace_membership(&window.foreign_toplevel)?,
        })
    }

    fn select_output(
        &self,
        candidates: &[wl_output::WlOutput],
        current_outputs: &std::collections::HashSet<wl_output::WlOutput>,
        requested_name: Option<&str>,
    ) -> Result<wl_output::WlOutput> {
        if let Some(requested_name) = requested_name {
            return candidates
                .iter()
                .find(|output| self.output_name(output).as_deref() == Some(requested_name))
                .cloned()
                .with_context(|| {
                    format!("target workspace group has no output named {requested_name:?}")
                });
        }
        if let Some(current) = candidates
            .iter()
            .find(|output| current_outputs.contains(*output))
        {
            return Ok(current.clone());
        }
        if let [only] = candidates {
            return Ok(only.clone());
        }
        bail!(
            "the target workspace group has {} possible outputs; pass --output <name>",
            candidates.len()
        )
    }

    fn workspace_membership(
        &self,
        foreign_window: &ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
    ) -> Result<Vec<WorkspaceId>> {
        let window = self
            .windows
            .iter()
            .find(|window| &window.foreign_toplevel == foreign_window)
            .context("the chosen Window closed while verifying its workspace move")?;
        if window.committed_workspaces.is_empty() {
            bail!(
                "failed workspace-move capability: COSMIC toplevel-info v3 advertised workspace membership but supplied none for Window {}",
                window.identifier
            );
        }
        Ok(self
            .membership_ids(&window.committed_workspaces)
            .into_iter()
            .map(WorkspaceId::from)
            .collect())
    }

    fn membership_ids(
        &self,
        handles: &std::collections::HashSet<ext_workspace_handle_v1::ExtWorkspaceHandleV1>,
    ) -> Vec<String> {
        let mut ids = handles
            .iter()
            .map(|handle| self.workspace_selector(handle))
            .collect::<Vec<_>>();
        ids.sort();
        ids
    }

    fn workspace_selector(&self, handle: &ext_workspace_handle_v1::ExtWorkspaceHandleV1) -> String {
        let Some(workspace) = self.workspace_state.workspace_info(handle) else {
            return format!("wayland:{:?}", handle.id());
        };
        if let Some(id) = &workspace.id {
            return format!("id:{id}");
        }
        let mut outputs = self
            .workspace_state
            .workspace_groups()
            .find(|group| group.workspaces.contains(handle))
            .map(|group| {
                group
                    .outputs
                    .iter()
                    .map(|output| self.output_label(output))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        outputs.sort();
        format!("name:{}@{}", workspace.name, outputs.join("+"))
    }

    fn output_name(&self, output: &wl_output::WlOutput) -> Option<String> {
        self.output_state.info(output).and_then(|info| info.name)
    }

    fn output_label(&self, output: &wl_output::WlOutput) -> String {
        self.output_name(output)
            .unwrap_or_else(|| format!("{:?}", output.id()))
    }
}

impl ProvidesRegistryState for WorkspaceMoveProbe {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers!(OutputState);
}

impl OutputHandler for WorkspaceMoveProbe {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl Dispatch<ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1, GlobalData>
    for WorkspaceMoveProbe
{
    fn event(
        state: &mut Self,
        list: &ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1,
        event: ext_foreign_toplevel_list_v1::Event,
        _data: &GlobalData,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
    ) {
        match event {
            ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } => {
                let cosmic_toplevel = state.cosmic_toplevel_info.get_cosmic_toplevel(
                    &toplevel,
                    queue_handle,
                    GlobalData,
                );
                state.windows.push(WorkspaceWindow {
                    foreign_toplevel: toplevel,
                    cosmic_toplevel,
                    identifier: String::new(),
                    app_id: String::new(),
                    outputs: std::collections::HashSet::new(),
                    workspaces: std::collections::HashSet::new(),
                    committed_workspaces: std::collections::HashSet::new(),
                    metadata_complete: false,
                });
            }
            ext_foreign_toplevel_list_v1::Event::Finished => list.destroy(),
            _ => {}
        }
    }

    wayland_client::event_created_child!(WorkspaceMoveProbe, ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1, [
        ext_foreign_toplevel_list_v1::EVT_TOPLEVEL_OPCODE => (ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1, ())
    ]);
}

impl Dispatch<ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1, ()>
    for WorkspaceMoveProbe
{
    fn event(
        state: &mut Self,
        toplevel: &ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
        event: ext_foreign_toplevel_handle_v1::Event,
        _data: &(),
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
        if let ext_foreign_toplevel_handle_v1::Event::Closed = event {
            state
                .windows
                .retain(|window| window.foreign_toplevel != *toplevel);
            return;
        }
        let Some(window) = state
            .windows
            .iter_mut()
            .find(|window| window.foreign_toplevel == *toplevel)
        else {
            return;
        };
        match event {
            ext_foreign_toplevel_handle_v1::Event::AppId { app_id } => {
                window.app_id = app_id;
            }
            ext_foreign_toplevel_handle_v1::Event::Identifier { identifier } => {
                window.identifier = identifier;
            }
            ext_foreign_toplevel_handle_v1::Event::Done => {
                window.metadata_complete = true;
            }
            _ => {}
        }
    }
}

impl Dispatch<zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1, GlobalData> for WorkspaceMoveProbe {
    fn event(
        state: &mut Self,
        _info: &zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1,
        event: zcosmic_toplevel_info_v1::Event,
        _data: &GlobalData,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
        if let zcosmic_toplevel_info_v1::Event::Done = event {
            for window in &mut state.windows {
                window.committed_workspaces.clone_from(&window.workspaces);
            }
            state.toplevel_snapshot_generation += 1;
        }
    }

    wayland_client::event_created_child!(WorkspaceMoveProbe, zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1, [
        zcosmic_toplevel_info_v1::EVT_TOPLEVEL_OPCODE => (zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1, GlobalData)
    ]);
}

impl Dispatch<zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1, GlobalData>
    for WorkspaceMoveProbe
{
    fn event(
        state: &mut Self,
        toplevel: &zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1,
        event: zcosmic_toplevel_handle_v1::Event,
        _data: &GlobalData,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
        let Some(window) = state
            .windows
            .iter_mut()
            .find(|window| window.cosmic_toplevel == *toplevel)
        else {
            return;
        };
        match event {
            zcosmic_toplevel_handle_v1::Event::OutputEnter { output } => {
                window.outputs.insert(output);
            }
            zcosmic_toplevel_handle_v1::Event::OutputLeave { output } => {
                window.outputs.remove(&output);
            }
            zcosmic_toplevel_handle_v1::Event::ExtWorkspaceEnter { workspace } => {
                window.workspaces.insert(workspace);
            }
            zcosmic_toplevel_handle_v1::Event::ExtWorkspaceLeave { workspace } => {
                window.workspaces.remove(&workspace);
            }
            _ => {}
        }
    }
}

impl WorkspaceHandler for WorkspaceMoveProbe {
    fn workspace_state(&mut self) -> &mut WorkspaceState {
        &mut self.workspace_state
    }

    fn done(&mut self) {
        self.workspace_snapshot_received = true;
    }
}

impl ToplevelManagerHandler for WorkspaceMoveProbe {
    fn toplevel_manager_state(&mut self) -> &mut ToplevelManagerState {
        &mut self.toplevel_manager_state
    }

    fn capabilities(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        capabilities: Vec<
            WEnum<zcosmic_toplevel_manager_v1::ZcosmicToplelevelManagementCapabilitiesV1>,
        >,
    ) {
        use zcosmic_toplevel_manager_v1::ZcosmicToplelevelManagementCapabilitiesV1 as Capability;

        let legacy = capabilities.contains(&WEnum::Value(Capability::MoveToWorkspace));
        let ext_workspace = capabilities.contains(&WEnum::Value(Capability::MoveToExtWorkspace));
        let raw = capabilities
            .into_iter()
            .map(|capability| match capability {
                WEnum::Value(capability) => capability as u32,
                WEnum::Unknown(value) => value,
            })
            .collect();
        self.management_capabilities = Some(AdvertisedManagementCapabilities {
            raw,
            workspace_move: WorkspaceMoveCapabilities::new(legacy, ext_workspace),
        });
    }
}

delegate_output!(WorkspaceMoveProbe);
delegate_registry!(WorkspaceMoveProbe);
cosmic_client_toolkit::delegate_toplevel_manager!(WorkspaceMoveProbe);
cosmic_client_toolkit::delegate_workspace!(WorkspaceMoveProbe);
