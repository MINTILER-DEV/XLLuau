#[derive(Debug, Clone)]
pub struct Program {
    pub block: Block,
}

pub type Block = Vec<Stmt>;

#[derive(Debug, Clone)]
pub enum Stmt {
    Local(LocalDecl),
    Function(FunctionDecl),
    Assignment(Assignment),
    NullishAssignment { target: AssignTarget, value: Expr },
    Call(Expr),
    Return(Vec<Expr>),
    If(IfStmt),
    While { condition: Expr, block: Block },
    Repeat { block: Block, condition: Expr },
    ForNumeric(ForNumeric),
    ForGeneric(ForGeneric),
    Do(Block),
    Break,
    Continue,
    TypeAlias { raw: String },
}

#[derive(Debug, Clone)]
pub struct LocalDecl {
    pub is_const: bool,
    pub bindings: Vec<Binding>,
    pub values: Vec<Expr>,
}

#[derive(Debug, Clone)]
pub struct Binding {
    pub pattern: Pattern,
    pub type_annotation: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Name(String),
    Table {
        entries: Vec<TablePatternEntry>,
        rest: Option<String>,
    },
    Array {
        items: Vec<ArrayPatternItem>,
        rest: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct TablePatternEntry {
    pub key: String,
    pub binding: PatternBinding,
}

#[derive(Debug, Clone)]
pub struct ArrayPatternItem {
    pub binding: Option<PatternBinding>,
}

#[derive(Debug, Clone)]
pub struct PatternBinding {
    pub target: Pattern,
    pub default_value: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct Assignment {
    pub targets: Vec<AssignTarget>,
    pub values: Vec<Expr>,
}

#[derive(Debug, Clone)]
pub enum AssignTarget {
    Name(String),
    Field { object: Box<Expr>, field: String },
    Index { object: Box<Expr>, index: Box<Expr> },
}

#[derive(Debug, Clone)]
pub struct FunctionDecl {
    pub local_name: bool,
    pub name: FunctionName,
    pub generics: Option<String>,
    pub params: Vec<Param>,
    pub return_type: Option<String>,
    pub body: Block,
}

#[derive(Debug, Clone)]
pub struct FunctionName {
    pub root: String,
    pub fields: Vec<String>,
    pub method: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Param {
    Binding(Binding),
    VarArg(Option<String>),
}

#[derive(Debug, Clone)]
pub struct IfStmt {
    pub branches: Vec<(Expr, Block)>,
    pub else_block: Option<Block>,
}

#[derive(Debug, Clone)]
pub struct ForNumeric {
    pub name: String,
    pub start: Expr,
    pub end: Expr,
    pub step: Option<Expr>,
    pub block: Block,
}

#[derive(Debug, Clone)]
pub struct ForGeneric {
    pub bindings: Vec<Binding>,
    pub iterables: Vec<Expr>,
    pub block: Block,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Nil,
    Bool(bool),
    Number(String),
    String(String),
    VarArg,
    Name(String),
    Table(Vec<TableField>),
    Function(FunctionExpr),
    IfElse {
        branches: Vec<(Expr, Expr)>,
        else_expr: Box<Expr>,
    },
    Paren(Box<Expr>),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },
    Ternary {
        condition: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
    },
    Chain {
        base: Box<Expr>,
        segments: Vec<ChainSegment>,
    },
    Pipe {
        left: Box<Expr>,
        stages: Vec<PipeStage>,
    },
}

#[derive(Debug, Clone)]
pub enum TableField {
    Named(String, Expr),
    Indexed(Expr, Expr),
    Value(Expr),
}

#[derive(Debug, Clone)]
pub struct FunctionExpr {
    pub generics: Option<String>,
    pub params: Vec<Param>,
    pub return_type: Option<String>,
    pub body: Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Negate,
    Not,
    Length,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Or,
    And,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Equal,
    NotEqual,
    Concat,
    Add,
    Subtract,
    Multiply,
    Divide,
    FloorDivide,
    Modulo,
    Power,
    Nullish,
}

#[derive(Debug, Clone)]
pub enum ChainSegment {
    Field {
        name: String,
        safe: bool,
    },
    Index {
        expr: Box<Expr>,
        safe: bool,
    },
    Call {
        args: Vec<Expr>,
    },
    MethodCall {
        name: String,
        args: Vec<Expr>,
        safe: bool,
    },
}

#[derive(Debug, Clone)]
pub enum PipeStage {
    Method { name: String, args: Vec<Expr> },
    Expr { callee: Box<Expr> },
    Call { callee: Box<Expr>, args: Vec<Expr> },
}
