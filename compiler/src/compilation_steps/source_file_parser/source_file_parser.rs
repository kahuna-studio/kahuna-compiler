use pest::error::Error;
use pest::iterators::Pairs;
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "compilation_steps/source_file_parser/kahuna.pest"]
struct KahunaParser;

pub fn parse_source_file(file_text: &str) -> Result<Pairs<'_, Rule>, Error<Rule>> {
  KahunaParser::parse(Rule::file, file_text)
}
