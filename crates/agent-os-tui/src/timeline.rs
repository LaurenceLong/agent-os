use crate::TuiProjection;

pub fn timeline_lines(projection: &TuiProjection, height: usize) -> Vec<String> {
    if projection.timeline.is_empty() {
        return vec!["No timeline items yet".to_string()];
    }
    let start = projection.timeline.len().saturating_sub(height.max(1));
    projection.timeline[start..].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeline_lines_keeps_tail() {
        let projection = TuiProjection {
            timeline: vec!["one".to_string(), "two".to_string(), "three".to_string()],
            ..TuiProjection::default()
        };

        assert_eq!(timeline_lines(&projection, 2), vec!["two", "three"]);
    }
}
