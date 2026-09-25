//! Text-row curve ingestion, ported from `app/modules/parser/text.go`.

use crate::curvebin::CurveData;
use crate::format::FormatDef;
use crate::logfile::LogFile;

impl LogFile {
    /// Ensures every field of a message type has a [`CurveData`] slot,
    /// creating the message bucket on first sight.
    pub(crate) fn init_curves(&mut self, msg_name: &str, fd: &FormatDef, instance: &str) {
        if self.curves.contains_key(msg_name) {
            return;
        }
        let mut fields = std::collections::BTreeMap::new();
        for fn_ in &fd.field_names {
            fields.insert(
                fn_.clone(),
                CurveData {
                    name: format!("{}.{}", msg_name, fn_),
                    instance: instance.to_string(),
                    ..Default::default()
                },
            );
        }
        self.curves.insert(msg_name.to_string(), fields);
    }

    /// Appends one text-parsed message instance to its curves, growing the
    /// times/samples vectors and tracking per-field min/max. Non-finite values
    /// are dropped so they never poison the chart range.
    ///
    /// `values` holds the already-tokenised text cells (strings), matching the
    /// Go text path where `ToFloat64` parses them defensively.
    pub fn store_text_curve_values(
        &mut self,
        msg_name: &str,
        fd: &FormatDef,
        values: &[TextValue],
        time_ms: f64,
        _lineno: usize,
        instance: &str,
    ) {
        self.init_curves(msg_name, fd, instance);
        for (i, fn_) in fd.field_names.iter().enumerate() {
            let Some(v) = values.get(i) else { break };
            let fval = v.to_f64();
            if !fval.is_finite() {
                continue;
            }
            let Some(cd) = self.curves.get_mut(msg_name).and_then(|m| m.get_mut(fn_)) else {
                continue;
            };
            cd.times.push(time_ms);
            cd.samples.push(fval as f32);
            if cd.times.len() == 1 {
                cd.min = fval;
                cd.max = fval;
            } else {
                if fval < cd.min {
                    cd.min = fval;
                }
                if fval > cd.max {
                    cd.max = fval;
                }
            }
        }
    }
}

/// One tokenised cell of a text log line. The Go original passes `[]any`
/// (strings for dataflash text, numbers elsewhere); parsing happens lazily in
/// [`TextValue::to_f64`] with the same NaN-on-failure semantics as
/// `parser.ToFloat64`.
#[derive(Debug, Clone)]
pub enum TextValue {
    Str(String),
    Num(f64),
}

impl TextValue {
    pub fn to_f64(&self) -> f64 {
        match self {
            TextValue::Num(v) => *v,
            TextValue::Str(s) => s.parse::<f64>().unwrap_or(f64::NAN),
        }
    }
}
