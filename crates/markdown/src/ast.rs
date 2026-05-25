/// Recursive Markdown AST.

#[derive(Debug, Clone, PartialEq)]
pub struct MarkdownDoc {
    pub frontmatter: Option<String>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading {
        level: u8,
        children: Vec<Inline>,
    },
    Paragraph(Vec<Inline>),
    CodeBlock {
        language: Option<String>,
        code: String,
    },
    List {
        items: Vec<ListItem>,
    },
    BlockQuote(Vec<Block>),
    Table {
        headers: Vec<Vec<Inline>>,
        aligns: Vec<Option<TableAlign>>,
        rows: Vec<Vec<Vec<Inline>>>,
    },
    ThematicBreak,
    MathBlock {
        content: String,
    },
    HtmlBlock {
        content: String,
    },
    FootnoteDefinition {
        label: String,
        content: Vec<Block>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text(String),
    Bold(Vec<Inline>),
    Italic(Vec<Inline>),
    Strikethrough(Vec<Inline>),
    Code(String),
    Link {
        text: Vec<Inline>,
        url: String,
        title: Option<String>,
    },
    Image {
        alt: String,
        url: String,
        title: Option<String>,
    },
    Math(String),
    FootnoteRef(String),
    HtmlTag(String),
    HardBreak,
    SoftBreak,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListItem {
    pub marker: ListMarker,
    pub children: Vec<Inline>,
    pub sub_items: Vec<ListItem>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ListMarker {
    Dash,
    Plus,
    Star,
    Number(u32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TableAlign {
    Left,
    Center,
    Right,
}
