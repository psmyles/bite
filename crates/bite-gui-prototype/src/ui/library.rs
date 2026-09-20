use bite_imgui::Ui;
use std::collections::BTreeMap;

pub const WORKFLOW_NODE_PAYLOAD: &str = "BITE_WORKFLOW_NODE";
pub const DEFINITION_NODE_PAYLOAD: &str = "BITE_DEFINITION_NODE";

#[derive(Clone, Debug)]
pub struct LibraryEntry {
    pub category: String,
    pub id: String,
    pub label: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkflowNode {
    Input,
    ImageOutput,
    TextOutput,
    FlipbookOutput,
    Comment,
}

impl WorkflowNode {
    pub fn payload(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::ImageOutput => "image-output",
            Self::TextOutput => "text-output",
            Self::FlipbookOutput => "flipbook-output",
            Self::Comment => "comment",
        }
    }

    pub fn from_payload(payload: &str) -> Option<Self> {
        match payload {
            "input" => Some(Self::Input),
            "image-output" => Some(Self::ImageOutput),
            "text-output" => Some(Self::TextOutput),
            "flipbook-output" => Some(Self::FlipbookOutput),
            "comment" => Some(Self::Comment),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Input => "Input",
            Self::ImageOutput => "Image Output",
            Self::TextOutput => "Text Output",
            Self::FlipbookOutput => "Flipbook Output",
            Self::Comment => "Comment",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LibraryAction {
    AddWorkflow(WorkflowNode),
    AddDefinition(String),
}

const WORKFLOW_NODES: [WorkflowNode; 5] = [
    WorkflowNode::Input,
    WorkflowNode::ImageOutput,
    WorkflowNode::TextOutput,
    WorkflowNode::FlipbookOutput,
    WorkflowNode::Comment,
];

pub fn draw(
    ui: &mut Ui<'_>,
    search: &mut String,
    entries: &[LibraryEntry],
) -> Option<LibraryAction> {
    ui.panel_header("Node Library");
    ui.next_item_full_width();
    ui.input_text("##library-search", search);

    let query = search.trim().to_lowercase();
    let mut action = None;

    if query.is_empty() && ui.collapsing_header("Workflow", true) {
        for node in WORKFLOW_NODES {
            if ui.selectable_drag_source(node.label(), WORKFLOW_NODE_PAYLOAD, node.payload()) {
                action = Some(LibraryAction::AddWorkflow(node));
            }
        }
    }

    let mut categories: BTreeMap<&str, Vec<&LibraryEntry>> = BTreeMap::new();
    for entry in entries.iter().filter(|entry| {
        query.is_empty()
            || entry.label.to_lowercase().contains(&query)
            || entry.category.to_lowercase().contains(&query)
            || entry.id.to_lowercase().contains(&query)
    }) {
        categories.entry(&entry.category).or_default().push(entry);
    }

    for (category, mut category_entries) in categories {
        category_entries.sort_by(|a, b| a.label.cmp(&b.label).then_with(|| a.id.cmp(&b.id)));
        if ui.collapsing_header(category, true) {
            for entry in category_entries {
                if ui.selectable_drag_source(&entry.label, DEFINITION_NODE_PAYLOAD, &entry.id) {
                    action = Some(LibraryAction::AddDefinition(entry.id.clone()));
                }
            }
        }
    }

    action
}
