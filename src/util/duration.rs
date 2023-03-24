use anyhow::{bail, ensure, Ok, Result};
use std::time::Duration;

pub(crate) trait DurationExt {
    fn to_secs_f64(self) -> f64;
}

impl DurationExt for Duration {
    fn to_secs_f64(self) -> f64 {
        self.as_secs() as f64 + self.subsec_nanos() as f64 * 1e-9
    }
}



pub(crate) fn parse(arg: &str) -> Result<Duration> {
    let uint = |arg: &str| arg.parse::<u32>().map(f64::from);
    let uf64 = |arg: &str| {
        let val: f64 = arg.parse()?;
        ensure!(val >= 0., "Negative duration is not allowed");
        Ok(val)
    };

    let segments: Vec<_> = arg.split(':').collect();

    let seconds = match segments.as_slice() {
        [seconds] => uf64(seconds)?,
        [minutes, seconds] => uint(minutes)? * 60. + uf64(seconds)?,
        [hours, minutes, seconds] => {
            uint(hours)? * (60. * 60.) + uint(minutes)? * 60. + uf64(seconds)?
        }
        _ => bail!("Unknown duration format"),
    };

    Ok(Duration::from_secs_f64(seconds))
}

#[cfg(test)]
mod tests {
    use super::*;
    use expect_test::expect;

    fn assert_parse(arg: &str, expected: expect_test::Expect) {
        expected.assert_eq(&format!("{:?}", parse(arg).unwrap()));
    }

    #[test]
    fn smoke_parse() {
        assert_parse("2", expect!["2s"]);
        assert_parse("1.234", expect!["1.234s"]);
        assert_parse("0.", expect!["0ns"]);

        assert_parse("12:45.4", expect!["765.4s"]);
        assert_parse("00:00.5", expect!["500ms"]);

        assert_parse("00:00:00.5", expect!["500ms"]);
        assert_parse("00:01:30.5", expect!["90.5s"]);
        assert_parse("00:20:10.5", expect!["1210.5s"]);
    }

    fn error_parse() {
        parse("-2").unwrap_err();
    }
}
