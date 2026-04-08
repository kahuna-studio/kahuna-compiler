use pest::error::Error;
use pest::iterators::Pairs;
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "compiler/lexer/kahuna.pest"]
struct KahunaParser;

pub fn lex(file_text: &str) -> Result<Pairs<'_, Rule>, Error<Rule>> {
  KahunaParser::parse(Rule::file, file_text)
}
