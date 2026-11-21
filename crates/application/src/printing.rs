//! Validated ranges for the reader's print adapter.
pub fn pages(input: &str, total: usize) -> Result<String, &'static str> {
    if total == 0 {
        return Err("El documento no tiene páginas para imprimir.");
    }
    let input = input.trim();
    if input.is_empty() {
        return Ok(if total == 1 {
            "1".into()
        } else {
            format!("1-{total}")
        });
    }
    if input.len() > 1024 {
        return Err("El rango es demasiado largo.");
    }
    let page = |value: &str| {
        value
            .trim()
            .parse::<usize>()
            .ok()
            .filter(|p| (1..=total).contains(p))
            .ok_or("Indica páginas válidas, por ejemplo 2,4-7.")
    };
    let mut ranges = Vec::new();
    for part in input.split(',') {
        let (begin, end) = match part.split_once('-') {
            Some((begin, end)) => (page(begin)?, page(end)?),
            None => {
                let page = page(part)?;
                (page, page)
            }
        };
        if end < begin {
            return Err("El inicio de un rango debe preceder a su final.");
        }
        ranges.push((begin, end));
    }
    ranges.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (begin, end) in ranges {
        if let Some(last) = merged.last_mut()
            && begin <= last.1.saturating_add(1)
        {
            last.1 = last.1.max(end);
        } else {
            merged.push((begin, end));
        }
    }
    Ok(merged
        .into_iter()
        .map(|(begin, end)| {
            if begin == end {
                begin.to_string()
            } else {
                format!("{begin}-{end}")
            }
        })
        .collect::<Vec<_>>()
        .join(","))
}

#[cfg(test)]
mod tests {
    use super::pages;

    #[test]
    fn physical_page_ranges_are_canonical_and_never_silently_clamped() {
        assert_eq!(pages("9,4-6,2,3,5-6", 12).unwrap(), "2-6,9");
        assert_eq!(pages("", 12).unwrap(), "1-12");
        assert_eq!(pages("", 1).unwrap(), "1");
        for invalid in ["0", "13", "3-2", "9-", "2-4-6", "3,"] {
            assert!(pages(invalid, 12).is_err(), "{invalid}");
        }
    }
}
