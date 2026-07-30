// SPDX-License-Identifier: GPL-3.0-only

use std::sync::{Arc, Mutex};

use accesskit::{
    ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, Node, NodeId, Role, Tree,
    TreeId, TreeUpdate,
};
use accesskit_unix::Adapter;
use cosmic_window_switcher::{Locale, StringKey, SwitcherGrid};

const ROOT_NODE_ID: NodeId = NodeId(0);

pub(super) struct AccessibilityBridge {
    adapter: Adapter,
    latest_tree: SharedTree,
    locale: Locale,
}

impl AccessibilityBridge {
    pub(super) fn new(locale: Locale) -> Self {
        let latest_tree = SharedTree::default();
        let adapter = Adapter::new(
            TreeActivation {
                latest_tree: latest_tree.clone(),
            },
            IgnoreActions,
            IgnoreDeactivation,
        );
        Self {
            adapter,
            latest_tree,
            locale,
        }
    }

    pub(super) fn set_locale(&mut self, locale: Locale) {
        self.locale = locale;
    }

    pub(super) fn update(&mut self, grid: &SwitcherGrid) {
        let update = tree_update(grid, self.locale);
        self.latest_tree.replace(Some(update.clone()));
        self.adapter.update_if_active(|| update);
        self.adapter.update_window_focus_state(true);
    }

    pub(super) fn hide(&mut self) {
        self.latest_tree.replace(None);
        self.adapter.update_window_focus_state(false);
        let locale = self.locale;
        self.adapter
            .update_if_active(move || hidden_tree_update(locale));
    }
}

#[derive(Clone, Default)]
struct SharedTree(Arc<Mutex<Option<TreeUpdate>>>);

impl SharedTree {
    fn get(&self) -> Option<TreeUpdate> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn replace(&self, update: Option<TreeUpdate>) {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = update;
    }
}

struct TreeActivation {
    latest_tree: SharedTree,
}

impl ActivationHandler for TreeActivation {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        self.latest_tree.get()
    }
}

struct IgnoreActions;

impl ActionHandler for IgnoreActions {
    fn do_action(&mut self, _request: ActionRequest) {}
}

struct IgnoreDeactivation;

impl DeactivationHandler for IgnoreDeactivation {
    fn deactivate_accessibility(&mut self) {}
}

fn tree_update(grid: &SwitcherGrid, locale: Locale) -> TreeUpdate {
    let item_ids = (1_u64..)
        .take(grid.items().len())
        .map(NodeId::from)
        .collect::<Vec<_>>();
    let mut root = Node::new(Role::ListBox);
    root.set_label(locale.text(StringKey::WindowSwitcher));
    root.set_description(locale.text(StringKey::InteractionInstructions));
    root.set_children(item_ids.clone());
    root.set_size_of_set(grid.items().len());

    let mut nodes = Vec::with_capacity(grid.items().len() + 1);
    nodes.push((ROOT_NODE_ID, root));
    let mut focus = ROOT_NODE_ID;
    for (item, node_id) in grid.items().iter().zip(item_ids) {
        let semantics = item.accessibility(locale);
        let mut node = Node::new(Role::ListBoxOption);
        node.set_label(semantics.name().to_owned());
        node.set_description(semantics.instructions().to_owned());
        node.set_position_in_set(semantics.position());
        node.set_size_of_set(semantics.set_size());
        node.set_selected(semantics.is_selected());
        if semantics.is_selected() {
            focus = node_id;
        }
        nodes.push((node_id, node));
    }

    let mut tree = Tree::new(ROOT_NODE_ID);
    tree.toolkit_name = Some("COSMIC Window Switcher".to_owned());
    TreeUpdate {
        nodes,
        tree: Some(tree),
        tree_id: TreeId::ROOT,
        focus,
    }
}

fn hidden_tree_update(locale: Locale) -> TreeUpdate {
    let mut root = Node::new(Role::ListBox);
    root.set_label(locale.text(StringKey::WindowSwitcher));
    TreeUpdate {
        nodes: vec![(ROOT_NODE_ID, root)],
        tree: Some(Tree::new(ROOT_NODE_ID)),
        tree_id: TreeId::ROOT,
        focus: ROOT_NODE_ID,
    }
}
