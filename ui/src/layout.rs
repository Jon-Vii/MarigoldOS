use display::Rect;

pub const MAX_NODES: usize = 64;
pub type NodeId = u8;
pub const NO_NODE: NodeId = u8::MAX;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NodeKind {
    #[default]
    Empty,
    Text,
    Rule,
    Progress,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextSpan {
    pub offset: u16,
    pub len: u16,
}

pub struct Layout<const N: usize> {
    pub kinds: [NodeKind; N],
    pub rects: [Rect; N],
    pub parents: [NodeId; N],
    pub text: [TextSpan; N],
    pub len: u8,
}

impl<const N: usize> Layout<N> {
    pub const fn new() -> Self {
        Self {
            kinds: [NodeKind::Empty; N],
            rects: [Rect::new(0, 0, 0, 0); N],
            parents: [NO_NODE; N],
            text: [TextSpan { offset: 0, len: 0 }; N],
            len: 0,
        }
    }

    pub fn push(
        &mut self,
        kind: NodeKind,
        rect: Rect,
        parent: NodeId,
        text: TextSpan,
    ) -> Option<NodeId> {
        let index = self.len as usize;
        if index >= N || index >= MAX_NODES {
            return None;
        }

        self.kinds[index] = kind;
        self.rects[index] = rect;
        self.parents[index] = parent;
        self.text[index] = text;
        self.len += 1;
        Some(index as NodeId)
    }
}

impl<const N: usize> Default for Layout<N> {
    fn default() -> Self {
        Self::new()
    }
}
