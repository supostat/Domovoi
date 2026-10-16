type DigitSequence = Vec<(u8, u8)>;

pub(crate) fn integer_range_expression(minimum: i64, maximum: i64) -> String {
    if minimum >= 0 {
        alternatives(&non_negative_range(minimum.unsigned_abs(), maximum.unsigned_abs()))
    } else if maximum < 0 {
        format!(
            "\"-\" {}",
            alternatives(&non_negative_range(maximum.unsigned_abs(), minimum.unsigned_abs()))
        )
    } else {
        format!(
            "(\"-\" {} | {})",
            alternatives(&non_negative_range(1, minimum.unsigned_abs())),
            alternatives(&non_negative_range(0, maximum.unsigned_abs()))
        )
    }
}

fn non_negative_range(minimum: u64, maximum: u64) -> Vec<DigitSequence> {
    let mut sequences = Vec::new();
    for length in digit_count(minimum)..=digit_count(maximum) {
        let shortest = if length == 1 { 0 } else { 10_u64.pow(length - 1) };
        let longest = 10_u64.checked_pow(length).map_or(u64::MAX, |power| power - 1);
        sequences.extend(same_length(
            &digits(minimum.max(shortest)),
            &digits(maximum.min(longest)),
        ));
    }
    sequences
}

fn same_length(low: &[u8], high: &[u8]) -> Vec<DigitSequence> {
    let Some((&first_low, rest_low)) = low.split_first() else {
        return vec![Vec::new()];
    };
    let (&first_high, rest_high) = high.split_first().expect("both bounds have the same length");
    if first_low == first_high {
        return prefixed((first_low, first_low), same_length(rest_low, rest_high));
    }
    let rest = rest_low.len();
    let low_is_full = rest_low.iter().all(|digit| *digit == 0);
    let high_is_full = rest_high.iter().all(|digit| *digit == 9);
    let mut sequences = Vec::new();
    if !low_is_full {
        sequences.extend(prefixed((first_low, first_low), same_length(rest_low, &vec![9; rest])));
    }
    let middle_start = if low_is_full { first_low } else { first_low + 1 };
    let middle_end = if high_is_full { first_high } else { first_high - 1 };
    if middle_start <= middle_end {
        let mut sequence = vec![(middle_start, middle_end)];
        sequence.extend(std::iter::repeat_n((0, 9), rest));
        sequences.push(sequence);
    }
    if !high_is_full {
        sequences.extend(prefixed(
            (first_high, first_high),
            same_length(&vec![0; rest], rest_high),
        ));
    }
    sequences
}

fn prefixed(first: (u8, u8), tails: Vec<DigitSequence>) -> Vec<DigitSequence> {
    tails
        .into_iter()
        .map(|tail| std::iter::once(first).chain(tail).collect())
        .collect()
}

fn alternatives(sequences: &[DigitSequence]) -> String {
    let rendered: Vec<String> = sequences.iter().map(|sequence| sequence_expression(sequence)).collect();
    if rendered.len() == 1 {
        rendered.into_iter().next().expect("one alternative")
    } else {
        format!("({})", rendered.join(" | "))
    }
}

fn sequence_expression(sequence: &[(u8, u8)]) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut literal = String::new();
    let mut position = 0;
    while position < sequence.len() {
        let (low, high) = sequence[position];
        if low == high {
            literal.push(char::from(b'0' + low));
            position += 1;
            continue;
        }
        if !literal.is_empty() {
            parts.push(format!("\"{literal}\""));
            literal.clear();
        }
        let run = sequence[position..]
            .iter()
            .take_while(|digits| **digits == (low, high))
            .count();
        let class = format!("[{low}-{high}]");
        parts.push(if run == 1 { class } else { format!("{class}{{{run}}}") });
        position += run;
    }
    if !literal.is_empty() {
        parts.push(format!("\"{literal}\""));
    }
    parts.join(" ")
}

fn digit_count(value: u64) -> u32 {
    value.checked_ilog10().map_or(1, |logarithm| logarithm + 1)
}

fn digits(value: u64) -> Vec<u8> {
    value.to_string().bytes().map(|digit| digit - b'0').collect()
}

#[cfg(test)]
mod tests {
    use super::{digits, non_negative_range};

    fn matches(sequences: &[Vec<(u8, u8)>], value: u64) -> bool {
        let value = digits(value);
        sequences.iter().any(|sequence| {
            sequence.len() == value.len()
                && sequence
                    .iter()
                    .zip(&value)
                    .all(|((low, high), digit)| (low..=high).contains(&digit))
        })
    }

    #[test]
    fn ranges_accept_exactly_their_members() {
        let bounds = [0, 1, 7, 9, 10, 19, 20, 99, 100, 101, 250, 999, 1000, 1234];
        for minimum in bounds {
            for maximum in bounds.into_iter().filter(|maximum| *maximum >= minimum) {
                let sequences = non_negative_range(minimum, maximum);
                for value in 0..=1500 {
                    assert_eq!(
                        matches(&sequences, value),
                        (minimum..=maximum).contains(&value),
                        "{value} in {minimum}..={maximum}: {sequences:?}"
                    );
                }
            }
        }
    }
}
