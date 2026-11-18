//! The sim's scoring as the plain trick readout ([`TrickText`]): the trick being composed, the sequence's points and
//! the line, its string ids read as words.
use engine_core::skate::TrickText;
use skate3_sim::physics::SkaterRuntime;

/// The readout for the skater's scoring now.
pub fn text(skater: &SkaterRuntime) -> TrickText {
    let input = skater.scoring.hud_input();
    let points = if input.sequence_score > 0 {
        if input.multiplier > 1.0 {
            format!("{} x{:.1}", input.sequence_score, input.multiplier)
        } else {
            input.sequence_score.to_string()
        }
    } else {
        String::new()
    };
    TrickText {
        name: readable_trick(&input.trick_name),
        points,
        line: if input.line_score > 0 { format!("line {}", input.line_score) } else { String::new() },
    }
}

/// The sim's trick names are its string ids and score tokens (`ID_TRICK_AIR_METRICS_FRONTFLIP`, `FS_BF_50_50_FSGRAB`):
/// shown as words (`Frontflip`, `Frontside 50-50 Frontside Grab`).
pub fn readable_trick(raw: &str) -> String {
    const DROP: [&str; 6] = ["ID", "TRICK", "TRICKS", "AIR", "METRICS", "NAME"];
    let word = |t: &str| -> String {
        match t {
            "BS" => "Backside".into(),
            "FS" => "Frontside".into(),
            "BF" => "".into(),
            "CROOK" => "Crooked".into(),
            "BOARD" => "Boardslide".into(),
            "LIP" => "Lipslide".into(),
            "NOSE" => "Nose".into(),
            "SHUVIT" | "SHOVEIT" => "Shove-it".into(),
            "POPSHUVIT" => "Pop Shove-it".into(),
            "FSGRAB" => "Frontside Grab".into(),
            "BSGRAB" => "Backside Grab".into(),
            "FSPOPSHUVIT" => "Frontside Pop Shove-it".into(),
            "BSPOPSHUVIT" => "Backside Pop Shove-it".into(),
            "FSSHUVIT" => "Frontside Shove-it".into(),
            "BSSHUVIT" => "Backside Shove-it".into(),
            _ => {
                let mut c = t.chars();
                match c.next() {
                    Some(f) if f.is_ascii_alphabetic() => f.to_string() + &c.as_str().to_ascii_lowercase(),
                    Some(f) => f.to_string() + c.as_str(),
                    None => String::new(),
                }
            }
        }
    };
    let mut out: Vec<String> = Vec::new();
    for part in raw.split(|c: char| c == '+' || c == '|') {
        let toks: Vec<&str> = part.split(|c: char| c == '_' || c.is_whitespace()).filter(|t| !t.is_empty()).collect();
        let mut words: Vec<String> = Vec::new();
        let mut i = 0;
        while i < toks.len() {
            let t = toks[i];
            if DROP.contains(&t) {
                i += 1;
                continue;
            }
            // 50_50 and 5_O read as 50-50 and 5-0
            if i + 1 < toks.len()
                && t.chars().all(|c| c.is_ascii_digit())
                && (toks[i + 1].chars().all(|c| c.is_ascii_digit()) || toks[i + 1] == "O")
            {
                let b = if toks[i + 1] == "O" { "0" } else { toks[i + 1] };
                words.push(format!("{t}-{b}"));
                i += 2;
                continue;
            }
            let w = word(t);
            if !w.is_empty() && words.last() != Some(&w) {
                words.push(w);
            }
            i += 1;
        }
        if !words.is_empty() {
            out.push(words.join(" "));
        }
    }
    out.join(" + ")
}

#[cfg(test)]
mod tests {
    #[test]
    fn port_ids_read_as_words() {
        assert_eq!(super::readable_trick("ID_TRICK_AIR_METRICS_FRONTFLIP"), "Frontflip");
        assert_eq!(super::readable_trick("BS_5_O"), "Backside 5-0");
        assert_eq!(super::readable_trick("FS_BF_50_50_FSGRAB"), "Frontside 50-50 Frontside Grab");
        assert_eq!(super::readable_trick("OLLIE"), "Ollie");
    }
}
