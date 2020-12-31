use logos::Logos;
use primitive_types::U256;

#[derive(Logos, Debug, PartialEq)]
/// A MelScript token.
pub enum Token {
    #[token("function")]
    KwFunction,
    #[token("scope")]
    KwScope,
    #[token("end")]
    KwEnd,
    #[token("using")]
    KwUsing,
    #[token("if")]
    KwIf,
    #[token("then")]
    KwThen,
    #[token("else")]
    KwElse,
    #[token("for")]
    KwFor,
    #[token("let")]
    KwLet,
    #[token("alias")]
    KwAlias,

    #[token("->")]
    ThinArrow,
    #[token("=>")]
    FatArrow,
    #[token(":")]
    Colon,
    #[token("::")]
    DoubleColon,
    #[token(".")]
    Dot,

    #[token("=")]
    Assigns,
    #[token("==")]
    Equals,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Times,
    #[token("/")]
    Divides,
    #[token("%")]
    Modulo,

    #[regex(r"[0-9]+", |lex| U256::from_dec_str(lex.slice()))]
    Natural(U256),
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    VarName(String),

    #[token("\n")]
    Newline,

    #[error]
    // We can also use this variant to define whitespace,
    // or any other matches we wish to skip.
    #[regex(r"[ \t\f]+", logos::skip)]
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple() {
        let mut lex = Token::lexer(
            r"using globals::CURRENT_TX
            using std
            
            function total_output[$OCOUNT]() -> Nat
                CURRENT_TX.outputs |>
                                limit[$OCOUNT]() |>
                                filter((coin: std::TxOutput) => coin.coin_type == std::TMEL) |>
                                map((coin: std::TxOutput) => coin.value)
            end
            
            total_output[$16]() % 2 == 1
    ",
        );
        let res: Vec<_> = lex.collect();
        dbg!(res);
    }
}
