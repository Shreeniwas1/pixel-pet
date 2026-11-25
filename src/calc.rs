//! The calculator's arithmetic: a small expression parser. Pure, so it can be tested.

/// Work out an expression such as `12*3.5 + 8%` or `sqrt(2)^2`. None if it makes no sense (yet).
pub(crate) fn eval(text: &str) -> Option<f64> {
    let cleaned: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ',' && *c != '_' && *c != '=')
        .map(|c| match c {
            '\u{d7}' | 'x' | 'X' => '*',
            '\u{f7}' => '/',
            '\u{2212}' => '-',
            c => c.to_ascii_lowercase(),
        })
        .collect();
    let mut p = Parser { s: cleaned.as_bytes(), i: 0, depth: 0 };
    let v = p.sum()?;
    (p.i == p.s.len() && v.is_finite()).then_some(v)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    depth: u32,
}

impl Parser<'_> {
    fn peek(&self) -> u8 {
        self.s.get(self.i).copied().unwrap_or(0)
    }

    fn eat(&mut self, c: u8) -> bool {
        let hit = self.peek() == c;
        self.i += hit as usize;
        hit
    }

    fn sum(&mut self) -> Option<f64> {
        let mut v = self.product()?.0;
        loop {
            let sign = match self.peek() {
                b'+' => 1.0,
                b'-' => -1.0,
                _ => return Some(v),
            };
            self.i += 1;
            let (w, percent) = self.product()?;
            // 200 + 10% means ten percent of the 200, as on a desk calculator
            v += sign * if percent { v * w } else { w };
        }
    }

    /// The value, and whether it was a bare percentage.
    fn product(&mut self) -> Option<(f64, bool)> {
        let (mut v, mut percent) = self.power()?;
        loop {
            match self.peek() {
                b'*' => {
                    self.i += 1;
                    v *= self.power()?.0;
                }
                b'/' => {
                    self.i += 1;
                    v /= self.power()?.0;
                }
                b'(' | b'a'..=b'z' => v *= self.power()?.0, // 2(3+4), 2pi
                _ => return Some((v, percent)),
            }
            percent = false;
        }
    }

    fn power(&mut self) -> Option<(f64, bool)> {
        self.depth += 1;
        if self.depth > 60 {
            return None;
        }
        let out = if self.eat(b'-') {
            let (v, p) = self.power()?;
            Some((-v, p))
        } else if self.eat(b'+') {
            self.power()
        } else {
            let (mut v, mut percent) = (self.atom()?, false);
            while self.eat(b'%') {
                v /= 100.0;
                percent = true;
            }
            if self.eat(b'^') {
                Some((v.powf(self.power()?.0), false))
            } else {
                Some((v, percent))
            }
        };
        self.depth -= 1;
        out
    }

    fn atom(&mut self) -> Option<f64> {
        if self.eat(b'(') {
            let v = self.sum()?;
            let _ = self.eat(b')'); // a forgotten last bracket is forgiven
            return Some(v);
        }
        let start = self.i;
        while self.peek().is_ascii_digit() || self.peek() == b'.' {
            self.i += 1;
        }
        if self.i > start {
            return std::str::from_utf8(&self.s[start..self.i]).ok()?.parse().ok();
        }
        while self.peek().is_ascii_alphabetic() {
            self.i += 1;
        }
        let name = &self.s[start..self.i];
        match name {
            b"pi" => return Some(std::f64::consts::PI),
            b"e" => return Some(std::f64::consts::E),
            _ => {}
        }
        if !self.eat(b'(') {
            return None;
        }
        let v = self.sum()?;
        let _ = self.eat(b')');
        Some(match name {
            b"sqrt" => v.sqrt(),
            b"abs" => v.abs(),
            b"round" => v.round(),
            b"sin" => v.to_radians().sin(),
            b"cos" => v.to_radians().cos(),
            b"tan" => v.to_radians().tan(),
            b"ln" => v.ln(),
            b"log" => v.log10(),
            _ => return None,
        })
    }
}

/// A result the way people write numbers: no stray 0.30000000000000004.
pub(crate) fn show(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    let a = v.abs();
    if !(1e-9..1e15).contains(&a) {
        return format!("{v:.9e}").replace("e", " e");
    }
    let places = (12 - a.log10().floor() as i32).clamp(0, 12) as usize;
    let s = format!("{v:.places$}");
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" { "0".into() } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is(text: &str, want: &str) {
        assert_eq!(eval(text).map(show).as_deref(), Some(want), "{text}");
    }

    #[test]
    fn arithmetic() {
        is("1+2*3", "7");
        is("(1+2)*3", "9");
        is("10/4", "2.5");
        is("2^3^2", "512");
        is("-3+5", "2");
        is("-2^2", "-4");
        is("2*-3", "-6");
        is("0.1+0.2", "0.3");
        is("1,250 x 4", "5000");
        is("7-10", "-3");
        is("1/3", "0.333333333333");
    }

    #[test]
    fn percentages_like_a_desk_calculator() {
        is("200+10%", "220");
        is("200-25%", "150");
        is("50%*80", "40");
        is("80*50%", "40");
        is("15%", "0.15");
    }

    #[test]
    fn functions_and_shorthand() {
        is("sqrt(16)", "4");
        is("sqrt(2)^2", "2");
        is("2(3+4)", "14");
        is("sin(30)", "0.5");
        is("round(pi)", "3");
        is("sqrt(9", "3");
        is("12*3=", "36");
    }

    #[test]
    fn nonsense_gives_no_answer_and_never_crashes() {
        for t in ["", "+", "1+", "1/0", "hello", "2**3", ")", "sqrt", "1..2", "%", "0/0", "sqrt(-1)", &"(".repeat(5000), &"-".repeat(5000)] {
            assert_eq!(eval(t), None, "{}", &t[..t.len().min(12)]);
        }
    }

    #[test]
    fn big_and_small_results_stay_readable() {
        is("10^20", "1.000000000 e20");
        is("2^40", "1099511627776");
        is("1/10^12", "1.000000000 e-12");
    }
}
