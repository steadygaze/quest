use askama_escape::{escape, Html};
use pest::iterators::Pair;
use pest::iterators::Pairs;
use pest::Parser;
use pest_derive::Parser;
use std::fmt::Write;

mod pretty_debug;

#[derive(Parser)]
#[grammar = "markup/markup.pest"]
pub struct MarkupParser;

type ParseError = pest::error::Error<Rule>;
type ParseResult = Result<String, ParseError>;

enum TraversalState<'a> {
    Prequeued(Pairs<'a, Rule>),
    Enter(Pair<'a, Rule>),
    FormatClose,
    BlockquoteClose,
    OutputChar(char),
    OutputStr(&'a str),
}

pub fn to_html(markup: &str) -> ParseResult {
    Ok(parsed_to_html(MarkupParser::parse(Rule::document, markup)?))
}

fn parsed_to_html(pairs: Pairs<Rule>) -> String {
    let mut html = String::new();
    let mut stack: Vec<TraversalState> = Vec::new();
    stack.push(TraversalState::Prequeued(pairs));
    let mut format_stack: Vec<&str> = Vec::new();
    let mut blockquote_last_depth: usize = 0;

    while stack.len() > 0 {
        let state = match stack.pop() {
            Some(tup) => tup,
            None => unreachable!(),
        };

        match state {
            TraversalState::Prequeued(pairs) => {
                let mut iter = pairs.into_iter().rev().peekable();
                while let Some(pair) = iter.next() {
                    let lf = match pair.as_rule() {
                        Rule::paragraph_line | Rule::list_line => iter.peek().is_some(),
                        // blockquote_depth is the first child of blockquote and
                        // doesn't count.
                        Rule::blockquote_line => iter
                            .peek()
                            .is_some_and(|next_pair| next_pair.as_rule() == Rule::blockquote_line),
                        _ => false,
                    };
                    stack.push(TraversalState::Enter(pair));
                    if lf {
                        stack.push(TraversalState::OutputChar('\n'));
                    }
                }
            }
            TraversalState::OutputChar(ch) => {
                html.push(ch);
            }
            TraversalState::OutputStr(str) => {
                html.push_str(str);
            }
            TraversalState::Enter(pair) => {
                let rule = pair.as_rule();
                match rule {
                    Rule::paragraph => {
                        html.push_str("<p>");
                        stack.push(TraversalState::OutputStr("</p>"));
                    }
                    Rule::unordered_list | Rule::ul_nested => {
                        html.push_str("<ul>");
                        stack.push(TraversalState::OutputStr("</ul>"));
                    }
                    Rule::ordered_list | Rule::ol_nested => {
                        html.push_str("<ol>");
                        stack.push(TraversalState::OutputStr("</ol>"));
                    }
                    Rule::list_item => {
                        html.push_str("<li>");
                        stack.push(TraversalState::OutputStr("</li>"));
                    }
                    Rule::control => match pair.as_str() {
                        "//" => {
                            html.push_str("<em>");
                            format_stack.push("</em>");
                        }
                        "**" => {
                            html.push_str("<strong>");
                            format_stack.push("</strong>");
                        }
                        "__" => {
                            html.push_str("<ins>");
                            format_stack.push("</ins>");
                        }
                        "~~" => {
                            html.push_str("<del>");
                            format_stack.push("</del>");
                        }
                        "||" => {
                            // Placeholder.
                            html.push_str("<span>");
                            format_stack.push("</span>");
                        }
                        str => {
                            panic!("Unknown format control string: {}", str);
                        }
                    },
                    Rule::blockquote_depth => {
                        let depth = pair.as_str().len();
                        if blockquote_last_depth < depth {
                            for _ in 0..(depth - blockquote_last_depth) {
                                html.push_str("<blockquote>");
                            }
                        } else {
                            for _ in 0..(blockquote_last_depth - depth) {
                                html.push_str("</blockquote>");
                            }
                        }
                        blockquote_last_depth = depth;
                    }
                    Rule::blockquote => {
                        stack.push(TraversalState::BlockquoteClose);
                    }
                    Rule::horizontal_rule => {
                        html.push_str("<hr />");
                    }
                    Rule::text => {
                        write!(&mut html, "{}", escape(pair.as_str(), Html))
                            .expect("escaping can't fail");
                    }
                    Rule::formatted
                    | Rule::formatted2
                    | Rule::formatted3
                    | Rule::formatted4
                    | Rule::formatted5
                    | Rule::formatted6 => {
                        stack.push(TraversalState::FormatClose);
                    }
                    _ => (),
                }
                let inner = pair.into_inner();
                if inner.len() > 0 {
                    stack.push(TraversalState::Prequeued(inner));
                }
            }
            TraversalState::FormatClose => {
                html.push_str(format_stack.pop().expect("Expected closing format string"));
            }
            TraversalState::BlockquoteClose => {
                for _ in 0..blockquote_last_depth {
                    html.push_str("</blockquote>");
                }
                blockquote_last_depth = 0;
            }
        }
    }

    html
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), ParseError>;

    // We could use the parses_to! macro provided by pest to assert on the
    // structure of the parse tree, however, we choose not to because it's not
    // part of the API contract and the actual measure of correctness is the
    // HTML output.

    /// Macro to check that an assertion is true. It's a macro to not throw off
    /// stack traces in test failures.
    macro_rules! assert_html {
        ($input:expr, $output:expr $(,)?) => {{
            let parse_tree = MarkupParser::parse(Rule::document, $input)?;
            eprintln!("{}", pretty_debug::stack_based(parse_tree.clone()));
            assert_eq!(parsed_to_html(parse_tree), $output);
            Ok(())
        }};
    }

    mod format {
        use super::*;
        #[test]
        fn text_basic() -> TestResult {
            assert_html!("hello", "<p>hello</p>")
        }

        #[test]
        fn bold_basic() -> TestResult {
            assert_html!("**bold**", "<p><strong>bold</strong></p>")
        }

        #[test]
        fn italic_basic() -> TestResult {
            assert_html!("//italic//", "<p><em>italic</em></p>")
        }

        #[test]
        fn underline_basic() -> TestResult {
            assert_html!("__underline__", "<p><ins>underline</ins></p>")
        }

        #[test]
        fn strikethrough_basic() -> TestResult {
            assert_html!("~~strikethrough~~", "<p><del>strikethrough</del></p>")
        }

        #[test]
        fn spoiler_basic() -> TestResult {
            assert_html!("||spoiler||", "<p><span>spoiler</span></p>")
        }

        #[test]
        fn max_nested() -> TestResult {
            assert_html!("**bold //italic __underline ~~strikethrough~~ ||spoiler text spoiler|| ~~strikethrough~~ underline__ italic// bold**", "<p><strong>bold <em>italic <ins>underline <del>strikethrough</del> <span>spoiler text spoiler</span> <del>strikethrough</del> underline</ins> italic</em> bold</strong></p>")
        }

        #[test]
        fn escape_all() -> TestResult {
            assert_html!("<>\"'", "<p>&lt;&gt;&quot;&#x27;</p>")
        }

        #[test]
        fn escaped_tag() -> TestResult {
            assert_html!(
                r#"<custom attr1="value">hello</custom>"#,
                "<p>&lt;custom attr1=&quot;value&quot;&gt;hello&lt;/custom&gt;</p>",
            )
        }
    }

    mod horizontal_rule {
        use super::*;

        #[test]
        fn basic() -> TestResult {
            assert_html!("---", "<hr />")
        }

        #[test]
        fn broken() -> TestResult {
            assert_html!("  --- ---   ", "<hr />")
        }

        #[test]
        fn weird() -> TestResult {
            assert_html!(" - -- ---", "<hr />")
        }
    }

    mod paragraph {
        use super::*;

        #[test]
        fn multi() -> TestResult {
            assert_html!(
                r#"hello

world"#,
                "<p>hello</p><p>world</p>",
            )
        }

        #[test]
        fn joined() -> TestResult {
            assert_html!(
                r#"hello
world"#,
                r#"<p>hello
world</p>"#,
            )
        }

        #[test]
        fn padded() -> TestResult {
            assert_html!(
                r#"

hello world


"#,
                "<p>hello world</p>",
            )
        }
    }

    mod blockquote {
        use super::*;

        #[test]
        fn basic() -> TestResult {
            assert_html!("> hello", "<blockquote>hello</blockquote>")
        }

        #[test]
        fn formatted() -> TestResult {
            assert_html!(
                "> hello //world//",
                "<blockquote>hello <em>world</em></blockquote>",
            )
        }

        #[test]
        fn continued() -> TestResult {
            assert_html!(
                r#"> hello
> continued"#,
                r#"<blockquote>hello
continued</blockquote>"#,
            )
        }

        #[test]
        fn strip_leading() -> TestResult {
            assert_html!(
                r#"  > hello
 > continued"#,
                r#"<blockquote>hello
continued</blockquote>"#,
            )
        }

        #[test]
        fn basic_nesting() -> TestResult {
            assert_html!(
                r#"> hello
> continued
>> nested"#,
                r#"<blockquote>hello
continued<blockquote>nested</blockquote></blockquote>"#,
            )
        }

        #[test]
        fn basic_return() -> TestResult {
            assert_html!(
                r#"> hello
> continued
>> nested
> return to original level"#,
                r#"<blockquote>hello
continued<blockquote>nested</blockquote>return to original level</blockquote>"#,
            )
        }

        #[test]
        fn nested_many() -> TestResult {
            assert_html!(
                ">>>>>>>>>>>>>> hello",
                "<blockquote><blockquote><blockquote><blockquote><blockquote><blockquote><blockquote><blockquote><blockquote><blockquote><blockquote><blockquote><blockquote><blockquote>hello</blockquote></blockquote></blockquote></blockquote></blockquote></blockquote></blockquote></blockquote></blockquote></blockquote></blockquote></blockquote></blockquote></blockquote>",
            )
        }

        #[test]
        fn triangular() -> TestResult {
            assert_html!(
                r#"> 1
>> 2
>>> 3
>>>> 4
>>>>> 5
>>>> 4
>>> 3
>> 2
> 1
"#,
                "<blockquote>1<blockquote>2<blockquote>3<blockquote>4<blockquote>5</blockquote>4</blockquote>3</blockquote>2</blockquote>1</blockquote>",
            )
        }

        #[test]
        fn multilevel() -> TestResult {
            assert_html!(
                r#"> 1
> 1
> 1
> 1
>> 2
>>> 3
>>> 3
>> 2
>> 2
>> 2
> 1
> 1
> 1
"#,
                r#"<blockquote>1
1
1
1<blockquote>2<blockquote>3
3</blockquote>2
2
2</blockquote>1
1
1</blockquote>"#,
            )
        }

        #[test]
        fn reentrant() -> TestResult {
            assert_html!(
                r#"> 1
> 1
>> 2
>>> 3
>>> 3
>> 2
>> 2
>>> 3
>> 2
>> 2
> 1
> 1
"#,
                r#"<blockquote>1
1<blockquote>2<blockquote>3
3</blockquote>2
2<blockquote>3</blockquote>2
2</blockquote>1
1</blockquote>"#,
            )
        }
    }

    mod list {
        use super::*;

        #[test]
        fn basic_ul() -> TestResult {
            assert_html!(
                r#"- hello
- world
- how
- are
- you
"#,
                "<ul><li>hello</li><li>world</li><li>how</li><li>are</li><li>you</li></ul>",
            )
        }

        #[test]
        fn basic_ol() -> TestResult {
            assert_html!(
                r#"1. hello
1. world
1. how
1. are
1. you
"#,
                "<ol><li>hello</li><li>world</li><li>how</li><li>are</li><li>you</li></ol>",
            )
        }

        #[test]
        fn formatted_ul() -> TestResult {
            assert_html!(
                r#"- hello //world//
- **how are you**
"#,
                "<ul><li>hello <em>world</em></li><li><strong>how are you</strong></li></ul>",
            )
        }

        #[test]
        fn formatted_ol() -> TestResult {
            assert_html!(
                r#"1. hello //world//
1. **how are you**
"#,
                "<ol><li>hello <em>world</em></li><li><strong>how are you</strong></li></ol>",
            )
        }

        #[test]
        fn single_ul() -> TestResult {
            assert_html!("- hello world", "<ul><li>hello world</li></ul>")
        }

        #[test]
        fn single_ol() -> TestResult {
            assert_html!("1. hello world", "<ol><li>hello world</li></ol>")
        }

        #[test]
        fn padded_ul() -> TestResult {
            assert_html!(
                "

                - hello
                - world",
                "<ul><li>hello</li><li>world</li></ul>",
            )
        }

        #[test]
        fn padded_ol() -> TestResult {
            assert_html!(
                "

                1. hello
                1. world",
                "<ol><li>hello</li><li>world</li></ol>",
            )
        }

        #[test]
        fn nested_ul() -> TestResult {
            assert_html!(
                r#"- hello
- world
  - how
  - are
  - you
"#,
                "<ul><li>hello</li><li>world</li><ul><li>how</li><li>are</li><li>you</li></ul></ul>",
            )
        }

        #[test]
        fn nested_return_ul() -> TestResult {
            assert_html!(
                r#"- hello
- world
  - how
  - are
- you
"#,
                "<ul><li>hello</li><li>world</li><ul><li>how</li><li>are</li></ul><li>you</li></ul>",
            )
        }

        #[test]
        fn nested_multi_ul() -> TestResult {
            assert_html!(
                r#"- hello
  - world
    - how
        - are
            - you
"#,
                "<ul><li>hello</li><ul><li>world</li></ul><ul><li>how</li></ul><ul><li>are</li></ul><ul><li>you</li></ul></ul>",
            )
        }
    }

    mod document {
        use super::*;

        #[test]
        fn document() -> TestResult {
            assert_html!(
                r#"Hello.
This is a completely normal document.
I have a //lot// of things to say.

1. Thing 1
1. Thing 2
1. Thing 3

What do you think?
"#,
                r#"<p>Hello.
This is a completely normal document.
I have a <em>lot</em> of things to say.</p><ol><li>Thing 1</li><li>Thing 2</li><li>Thing 3</li></ol><p>What do you think?</p>"#,
            )
        }
    }
}
