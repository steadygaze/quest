use pest::iterators::Pair;
use pest::iterators::Pairs;
use pest::RuleType;
use std::fmt::Write;

enum TraversalState<T1, T2> {
    Start(T1),
    Middle(T2),
    End,
}

pub fn stack_based<R: RuleType>(pairs: Pairs<R>) -> String {
    let mut debug_repr = String::new();
    let mut stack: Vec<(u16, TraversalState<Pairs<R>, Pair<R>>)> = Vec::new();
    stack.push((0, TraversalState::Start(pairs)));

    while stack.len() > 0 {
        let (level, state) = match stack.pop() {
            Some(tup) => tup,
            None => unreachable!(),
        };

        match state {
            TraversalState::Start(pairs) => {
                for pair in pairs.into_iter().rev() {
                    stack.push((level, TraversalState::Middle(pair)));
                }
            }
            TraversalState::Middle(pair) => {
                for _ in 0..level {
                    debug_repr.push_str("    ");
                }
                write!(&mut debug_repr, "{:?}", pair.as_rule()).unwrap();
                debug_repr.push('(');
                write!(&mut debug_repr, "{}", pair.as_span().start()).unwrap();
                debug_repr.push_str(", ");
                write!(&mut debug_repr, "{}", pair.as_span().end()).unwrap();
                let inner = pair.into_inner();
                if inner.len() <= 0 {
                    debug_repr.push_str("),\n");
                } else {
                    debug_repr.push_str(", [\n");
                    stack.push((level, TraversalState::End));
                    stack.push((level + 1, TraversalState::Start(inner)));
                }
            }
            TraversalState::End => {
                for _ in 0..level {
                    debug_repr.push_str("    ");
                }
                debug_repr.push_str("]),\n");
            }
        }
    }

    debug_repr
}

pub fn recursive<R: RuleType>(pairs: Pairs<R>) -> String {
    let mut debug_repr = String::new();
    recursive_helper(&mut debug_repr, 0, pairs);
    debug_repr
}

fn recursive_helper<R: RuleType>(buf: &mut String, level: u8, pairs: Pairs<R>) {
    for pair in pairs.into_iter() {
        for _ in 0..level {
            buf.push_str("    ");
        }
        write!(buf, "{:?}", pair.as_rule()).unwrap();
        // println!("Initial traversal of {:?}", pair.as_rule());
        buf.push('(');
        write!(buf, "{}", pair.as_span().start()).unwrap();
        buf.push_str(", ");
        write!(buf, "{}", pair.as_span().end()).unwrap();
        let inner = pair.into_inner();
        if inner.len() <= 0 {
            buf.push_str("),\n");
        } else {
            buf.push_str(", [\n");
            recursive_helper(buf, level + 1, inner);

            for _ in 0..level {
                buf.push_str("    ");
            }
            buf.push_str("]),\n");
        }
    }
}
