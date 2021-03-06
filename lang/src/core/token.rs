use std::fmt::{Display, Formatter, Result as FmtResult};

use internment::Intern;

use super::{span::Span, Name};

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub enum Token {
    Variable,
    Loop,
    Break,
    Continue,
    Return,
    Function,
    Structure,
    If,
    Else,
    Use,
    Add,
    Minus,
    Multiply,
    Divide,
    Modulo,
    Lesser,
    Greater,
    LesserOrEqual,
    GreaterOrEqual,
    Equal,
    NotEqual,
    Or,
    And,
    Assign,
    Reference,
    Specify,
    Of,
    Arrow,
    ModuleAcess,
    Terminate,
    Separate,
    Quote,
    CurlyLeft,
    CurlyRight,
    Left,
    Right,
    Name(Name),
    String(Intern<String>), // I could use Name for this, but I think this more clearly conveys the meaning of the data being stored.
    Int(i32),
    Boolean(bool),
    EndOfFile,
    Error,
}

impl Default for Token {
    fn default() -> Self {
        Self::Error
    }
}

type StringElement = Vec<(MetaToken, Span)>;

#[derive(Debug, Clone)]
pub enum MetaToken {
    Token(Token),
    FormatString(Vec<(StringElement, Span)>),
    Block(Vec<(MetaToken, Span)>),
}

impl Display for Token {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Token::Variable => "`var`",
            Token::Loop => "`loop`",
            Token::Break => "`break`",
            Token::Continue => "`continue`",
            Token::Return => "`return`",
            Token::Function => "`func`",
            Token::Structure => "`struct`",
            Token::If => "`if`",
            Token::Else => "`else`",
            Token::Use => "`use`",
            Token::Add => "`+`",
            Token::Minus => "`-`",
            Token::Multiply => "`*`",
            Token::Divide => "`/`",
            Token::Modulo => "`%`",
            Token::LesserOrEqual => "`<=`",
            Token::GreaterOrEqual => "`>=`",
            Token::Equal => "`==`",
            Token::NotEqual => "`!=`",
            Token::Or => "`||`",
            Token::And => "`&&`",
            Token::Assign => "`=`",
            Token::Reference => "`&`",
            Token::Specify => "`:`",
            Token::Of => "`.`",
            Token::Arrow => "`->`",
            Token::ModuleAcess => "`::`",
            Token::Terminate => "`;`",
            Token::Lesser => "`<`",
            Token::Greater => "`>`",
            Token::Left => "`(`",
            Token::Right => "`)`",
            Token::Separate => "`,`",
            Token::CurlyLeft => "`{`",
            Token::CurlyRight => "`}`",
            Token::Quote => "`\"`",
            Token::Int(_) => "integer",
            Token::Boolean(_) => "boolean",
            Token::String(_) => "string",
            Token::Name(_) => "identifier",
            Token::EndOfFile => "end of file",
            _ => unreachable!(),
        })
    }
}

impl Display for MetaToken {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            MetaToken::Token(token) => token.fmt(f),
            MetaToken::FormatString(_) => f.write_str("string"),
            MetaToken::Block(_) => f.write_str("block"),
        }
    }
}
