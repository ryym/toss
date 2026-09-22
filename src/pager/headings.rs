use std::ops::Range;

use crate::{document::Document, pager::options::HeadingOptions};

/// The heading pattern plus a memo of where the heading start lines are.
///
/// Composing a page needs the nearest heading start at or above a given line, which is a
/// backward scan over the document. Without a memo that scan is proportional to the
/// document length on every frame, and a document with no heading in it pays the worst
/// case every time.
///
/// A recorded answer never goes stale: lines are immutable and the document only appends,
/// so a line that starts a heading keeps starting one. The end of a growing document is
/// the one place that does not hold yet, so those lines are scanned without being
/// recorded.
#[derive(Debug)]
pub(super) struct Headings {
    options: HeadingOptions,
    /// Confirmed heading start lines, ascending.
    starts: Vec<usize>,
    /// Lines already tested, as one contiguous range. `starts` is complete within it, so
    /// this is exactly where the memo can also answer "no heading start here".
    tested: Range<usize>,
}

impl Headings {
    pub fn new(options: HeadingOptions) -> Self {
        Self {
            options,
            starts: Vec::new(),
            tested: 0..0,
        }
    }

    /// Return how many lines a heading block spans (`--heading-lines`).
    pub fn num_lines(&self) -> usize {
        self.options.num_lines
    }

    /// Return whether the line at `line_index` starts a heading block.
    ///
    /// Example: with `toss --heading '^#' --heading-lines 2`
    /// ```text
    /// # title     => true
    /// sub title   => false (not a start line)
    /// other line  => false
    /// ```
    ///
    /// Even when a line matches the heading pattern, if another line within the following
    /// `--heading-lines` lines also matches, the earlier line is NOT a heading start.
    ///
    /// Example: with `toss --heading '^#' --heading-lines 2`
    /// ```text
    /// # title 1    => Not a heading as there is `## title 2`
    /// ## title 2   => Not a heading as there is `### title 3`
    /// ### title 3  => A heading and a heading start
    /// sentence 1   => A part of the heading but not a start line
    /// sentence 2   => Not a heading
    /// ```
    ///
    /// A line that does not exist cannot match, so the last lines of a growing document are
    /// heading starts merely because nothing has arrived after them. Their answer changes
    /// once it does.
    ///
    /// Example: with `toss --heading '^#' --heading-lines 2`, while only two lines have
    /// arrived
    /// ```text
    /// # title 1    => Not a heading as there is `# title 2`
    /// # title 2    => A heading as nothing follows it yet
    /// ```
    /// and after the next line arrives
    /// ```text
    /// # title 1    => Not a heading as there is `# title 2`
    /// # title 2    => No longer a heading as there is now `# title 3`
    /// # title 3    => A heading as nothing follows it yet
    /// ```
    ///
    /// Asking about a single line takes no shortcut: only
    /// [`Self::start_at_or_above`] consults the memo.
    pub fn is_start(&self, doc: &mut Document, line_index: usize) -> bool {
        match doc.line(line_index) {
            Some(line) if line.has_match(&self.options.pattern) => {}
            _ => return false,
        }
        for i in 1..self.options.num_lines {
            match doc.line(line_index + i) {
                Some(line) => {
                    if line.has_match(&self.options.pattern) {
                        return false;
                    }
                }
                None => return true,
            }
        }
        true
    }

    /// Find the nearest heading start in `first_candidate..=at`, touching the document only
    /// for the lines the memo cannot answer.
    pub fn start_at_or_above(
        &mut self,
        doc: &mut Document,
        first_candidate: usize,
        at: usize,
    ) -> Option<usize> {
        if at < first_candidate {
            return None;
        }
        let settled_end = if doc.is_complete() {
            usize::MAX
        } else {
            doc.line_count()
                .saturating_sub(self.options.num_lines.saturating_sub(1))
        };

        let mut line = at;
        let found = loop {
            if self.tested.contains(&line) {
                // The memo covers this line down to `tested.start`.
                let from = self.tested.start.max(first_candidate);
                if let Some(start) = self.recorded_start_in(from..(line + 1)) {
                    break Some(start);
                }
                if self.tested.start <= first_candidate {
                    break None;
                }
                // The memo ran out above `first_candidate`: keep scanning below it.
                line = self.tested.start - 1;
                continue;
            }
            if self.is_start(doc, line) {
                // Memoize only lines whose following lines have all arrived; for the rest,
                // is_start can still change its answer as the document grows.
                if line < settled_end {
                    self.record_start(line);
                }
                break Some(line);
            }
            if line == first_candidate {
                break None;
            }
            line -= 1;
        };
        // Everything from `line` up to `at` has now been tested, one way or another.
        self.mark_tested(line..(at + 1).min(settled_end));
        found
    }

    /// Find the nearest heading start strictly below `at`, never above `first_candidate`.
    pub fn start_below(
        &self,
        doc: &mut Document,
        first_candidate: usize,
        at: usize,
    ) -> Option<usize> {
        // No memo here: unlike start_at_or_above, which runs on every frame, this runs only
        // once per key press.
        let mut line = (at + 1).max(first_candidate);
        while doc.line(line).is_some() {
            if self.is_start(doc, line) {
                return Some(line);
            }
            line += 1;
        }
        None
    }

    /// Return the greatest recorded start within `range`.
    fn recorded_start_in(&self, range: Range<usize>) -> Option<usize> {
        let end = self.starts.partition_point(|&start| start < range.end);
        let start = *self.starts[..end].last()?;
        (start >= range.start).then_some(start)
    }

    /// Record `line_index` as a heading start, keeping [`Self::starts`] ascending.
    /// Recording a line already known to be a start does nothing.
    fn record_start(&mut self, line_index: usize) {
        if let Err(i) = self.starts.binary_search(&line_index) {
            self.starts.insert(i, line_index);
        }
    }

    /// Record that every line in `range` has been tested, so the memo can also answer
    /// "no heading start here" for them.
    ///
    /// The tested lines are tracked as a single range. A `range` that does not touch the
    /// current one therefore replaces it instead of extending it, and the lines the memo
    /// used to cover stop being answerable. An empty `range` records nothing.
    fn mark_tested(&mut self, range: Range<usize>) {
        if range.start >= range.end {
            return;
        }
        let overlaps = range.start <= self.tested.end && self.tested.start <= range.end;
        self.tested = if self.tested.is_empty() || !overlaps {
            // Only the tested range is given up; the recorded starts stay valid either way.
            range
        } else {
            self.tested.start.min(range.start)..self.tested.end.max(range.end)
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::StreamMsg;
    use crate::line::Line;
    use regex::Regex;
    use std::sync::mpsc;

    fn heading_options(pattern: &str, num_lines: usize) -> HeadingOptions {
        HeadingOptions {
            pattern: Regex::new(pattern).unwrap(),
            num_lines,
        }
    }

    /// 20 lines with a heading at 0, 5 and 12.
    fn doc_with_headings() -> Document {
        let mut lines: Vec<String> = (0..20).map(|i| format!("line{i}")).collect();
        for i in [0, 5, 12] {
            lines[i] = format!("# h{i}");
        }
        Document::from_string(lines.join("\n"))
    }

    #[test]
    fn the_heading_memo_answers_like_a_full_scan() {
        let mut doc = doc_with_headings();
        let mut headings = Headings::new(heading_options("^# ", 1));
        // A second instance answers one line at a time, with no memo of its own to lean on.
        let probe = Headings::new(heading_options("^# ", 1));

        // Query out of order, so the memo also meets lines far below what it has scanned.
        for at in [19, 18, 3, 4, 13, 12, 11, 6, 0, 19] {
            let expected = (0..=at).rev().find(|&i| probe.is_start(&mut doc, i));
            assert_eq!(
                headings.start_at_or_above(&mut doc, 0, at),
                expected,
                "at line {at}"
            );
        }
    }

    #[test]
    fn the_heading_memo_never_reaches_below_the_lower_bound() {
        let mut doc = doc_with_headings();
        let mut headings = Headings::new(heading_options("^# ", 1));
        assert_eq!(headings.start_at_or_above(&mut doc, 0, 19), Some(12));
        assert_eq!(headings.start_at_or_above(&mut doc, 6, 11), None);
        assert_eq!(headings.start_at_or_above(&mut doc, 6, 19), Some(12));
        assert_eq!(headings.start_at_or_above(&mut doc, 1, 4), None);
    }

    #[test]
    fn a_heading_start_at_the_end_of_a_growing_document_is_not_memoized() {
        let (tx, rx) = mpsc::channel();
        let mut doc = Document::from_channel(rx);
        tx.send(StreamMsg::Line(Line::new(0, "# a".into())))
            .unwrap();
        doc.pump();

        // With --heading-lines 2, "# a" is a heading start only while the line that would
        // follow it is still unknown.
        let mut headings = Headings::new(heading_options("^# ", 2));
        assert_eq!(headings.start_at_or_above(&mut doc, 0, 0), Some(0));

        tx.send(StreamMsg::Line(Line::new(1, "# b".into())))
            .unwrap();
        doc.pump();
        assert_eq!(headings.start_at_or_above(&mut doc, 0, 0), None);
    }

    #[test]
    fn start_below_finds_the_nearest_heading_start_after_the_line() {
        let mut doc = doc_with_headings();
        let headings = Headings::new(heading_options("^# ", 1));
        for (at, expected) in [(0, Some(5)), (4, Some(5)), (5, Some(12)), (11, Some(12))] {
            assert_eq!(
                headings.start_below(&mut doc, 0, at),
                expected,
                "at line {at}"
            );
        }
    }

    #[test]
    fn start_below_never_returns_a_line_above_the_lower_bound() {
        let mut doc = doc_with_headings();
        let headings = Headings::new(heading_options("^# ", 1));
        // Lines 0..6 are the global header: the heading at line 5 is not a candidate.
        assert_eq!(headings.start_below(&mut doc, 6, 0), Some(12));
    }

    #[test]
    fn start_below_returns_none_without_a_heading_below() {
        let mut doc = doc_with_headings();
        let headings = Headings::new(heading_options("^# ", 1));
        assert_eq!(headings.start_below(&mut doc, 0, 12), None);
        assert_eq!(headings.start_below(&mut doc, 0, 19), None);
    }
}
