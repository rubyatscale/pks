use crate::packs::pack::Pack;

/// Returns `contents` with `names` removed from its top-level `dependencies:`
/// list, edited as text so comments, key order and line endings survive.
///
/// Returns `None` when the edited text does not parse back to the original
/// pack minus exactly `names`, which happens when the list uses a layout the
/// line matcher does not recognise (flow style, multi-line scalars, ...).
pub(super) fn remove_dependencies(
    contents: &str,
    names: &[String],
) -> Option<String> {
    let edited = remove_dependency_lines(contents, names);
    is_exact_removal(contents, &edited, names).then_some(edited)
}

fn is_exact_removal(original: &str, edited: &str, names: &[String]) -> bool {
    let (Ok(mut expected), Ok(actual)) = (
        yaml_serde::from_str::<Pack>(original),
        yaml_serde::from_str::<Pack>(edited),
    ) else {
        return false;
    };
    expected.dependencies.retain(|name| !names.contains(name));
    expected == actual
}

/// Drops each matching item. A comment covers the items below it, up to the
/// next blank line or comment, and is dropped only when all of them are,
/// unless it holds a commented-out entry. If no items are left, the whole
/// block goes, as serializing the pack would do. Everything else is verbatim.
fn remove_dependency_lines(contents: &str, names: &[String]) -> String {
    let lines: Vec<&str> = contents.split_inclusive('\n').collect();
    let Some(key) = lines.iter().position(|line| is_dependencies_key(line))
    else {
        return contents.to_string();
    };
    let mut block_end = key + 1;
    while lines.get(block_end).is_some_and(|line| {
        let trimmed = line.trim_start();
        trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.starts_with("- ")
    }) {
        block_end += 1;
    }
    // Comments directly above the next key belong to the key.
    let mut tail = block_end;
    while tail > key + 1 && lines[tail - 1].trim_start().starts_with('#') {
        tail -= 1;
    }

    let block = edit_block(&lines[key + 1..tail], names);
    let mut out: Vec<&str> = lines[..key].to_vec();
    if let Some(block) = block {
        out.push(lines[key]);
        out.extend(block);
    }
    out.extend(&lines[tail..]);
    out.concat()
}

/// The block's lines with the named items removed, or `None` if no items are
/// left.
fn edit_block<'a>(lines: &[&'a str], names: &[String]) -> Option<Vec<&'a str>> {
    let mut out: Vec<&'a str> = Vec::new();
    let mut group = Group::default();
    let mut kept_items = 0;

    for &line in lines {
        let trimmed = line.trim_start();
        if trimmed.starts_with("- ") {
            group.items.push(line);
            continue;
        }
        if trimmed.starts_with('#') && group.items.is_empty() {
            group.comments.push(line);
            continue;
        }
        let vanished = group.flush(&mut out, names, &mut kept_items);
        if trimmed.starts_with('#') {
            group.comments.push(line);
        } else if !(vanished && out.last().is_some_and(|l| l.trim().is_empty()))
        {
            // Skipped when a removed group sat between two blank lines.
            out.push(line);
        }
    }
    group.flush(&mut out, names, &mut kept_items);
    (kept_items > 0).then_some(out)
}

/// A run of comment lines and the items below them.
#[derive(Default)]
struct Group<'a> {
    comments: Vec<&'a str>,
    items: Vec<&'a str>,
}

impl<'a> Group<'a> {
    /// Moves the group's surviving lines to `out`, returning whether it had
    /// lines and none survived.
    fn flush(
        &mut self,
        out: &mut Vec<&'a str>,
        names: &[String],
        kept_items: &mut usize,
    ) -> bool {
        let had_lines = !self.comments.is_empty() || !self.items.is_empty();
        let had_items = !self.items.is_empty();
        let kept: Vec<&'a str> = self
            .items
            .drain(..)
            .filter(|line| !is_named_item(line, names))
            .collect();
        *kept_items += kept.len();
        let keep_comments = !had_items
            || !kept.is_empty()
            || self.comments.iter().any(|line| is_commented_out_item(line));
        if !keep_comments {
            self.comments.clear();
        }
        let vanished = had_lines && self.comments.is_empty() && kept.is_empty();
        out.append(&mut self.comments);
        out.extend(kept);
        vanished
    }
}

fn is_named_item(line: &str, names: &[String]) -> bool {
    let item = &line.trim_start()["- ".len()..];
    names.iter().any(|name| name == item_value(item))
}

fn is_commented_out_item(line: &str) -> bool {
    line.trim_start()
        .strip_prefix('#')
        .is_some_and(|rest| rest.trim_start().starts_with("- "))
}

/// Matches the top-level key only: a nested `dependencies:` is indented.
fn is_dependencies_key(line: &str) -> bool {
    strip_comment(line).trim_end() == "dependencies:"
}

/// The scalar value of a block list item, without quotes or a trailing
/// comment. Escapes inside quotes are not decoded; the parse check in
/// `remove_dependencies` catches any item that this misreads.
fn item_value(item: &str) -> &str {
    let item = item.trim();
    for quote in ['"', '\''] {
        if let Some(rest) = item.strip_prefix(quote) {
            return rest.find(quote).map_or(item, |end| &rest[..end]);
        }
    }
    strip_comment(item).trim_end()
}

/// In YAML, `#` starts a comment only at the start of a line or after
/// whitespace, so `packs/a#b` is a plain value.
fn strip_comment(text: &str) -> &str {
    text.match_indices('#')
        .find(|(index, _)| *index == 0 || text[..*index].ends_with([' ', '\t']))
        .map_or(text, |(index, _)| &text[..index])
}

#[cfg(test)]
mod tests {
    use super::remove_dependencies;
    use pretty_assertions::assert_eq;

    fn remove(contents: &str, names: &[&str]) -> Option<String> {
        let names: Vec<String> = names.iter().map(|n| n.to_string()).collect();
        remove_dependencies(contents, &names)
    }

    fn assert_removes(before: &str, names: &[&str], after: &str) {
        assert_eq!(remove(before, names).as_deref(), Some(after));
    }

    #[test]
    fn removes_first_middle_and_last_items() {
        let before = "dependencies:\n- packs/a\n- packs/b\n- packs/c\n";
        assert_removes(
            before,
            &["packs/a"],
            "dependencies:\n- packs/b\n- packs/c\n",
        );
        assert_removes(
            before,
            &["packs/b"],
            "dependencies:\n- packs/a\n- packs/c\n",
        );
        assert_removes(
            before,
            &["packs/c"],
            "dependencies:\n- packs/a\n- packs/b\n",
        );
        assert_removes(
            before,
            &["packs/a", "packs/c"],
            "dependencies:\n- packs/b\n",
        );
    }

    #[test]
    fn removes_the_key_when_no_items_remain() {
        assert_removes(
            "enforce_dependencies: true\ndependencies:\n- packs/a\n- packs/b\nlayer: utilities\n",
            &["packs/a", "packs/b"],
            "enforce_dependencies: true\nlayer: utilities\n",
        );
        assert_removes("dependencies:\n- packs/a\n", &["packs/a"], "");
        assert_removes(
            "# Header.\ndependencies:\n- packs/a\n",
            &["packs/a"],
            "# Header.\n",
        );
    }

    #[test]
    fn handles_dependencies_as_the_last_key() {
        assert_removes(
            "enforce_privacy: true\ndependencies:\n- packs/a\n- packs/b\n",
            &["packs/b"],
            "enforce_privacy: true\ndependencies:\n- packs/a\n",
        );
        assert_removes(
            "enforce_privacy: true\ndependencies:\n- packs/a\n- packs/b",
            &["packs/b"],
            "enforce_privacy: true\ndependencies:\n- packs/a\n",
        );
    }

    #[test]
    fn handles_indented_lists() {
        assert_removes(
            "dependencies:\n  - packs/a\n  - packs/b\nlayer: utilities\n",
            &["packs/a"],
            "dependencies:\n  - packs/b\nlayer: utilities\n",
        );
    }

    #[test]
    fn handles_quoted_items() {
        assert_removes(
            "dependencies:\n- \"packs/a\"\n- 'packs/b'\n- packs/c\n",
            &["packs/a", "packs/b"],
            "dependencies:\n- packs/c\n",
        );
    }

    #[test]
    fn handles_a_trailing_comment_on_an_item() {
        assert_removes(
            "dependencies:\n- packs/a  # used\n- packs/b  # unused\n",
            &["packs/b"],
            "dependencies:\n- packs/a  # used\n",
        );
    }

    #[test]
    fn keeps_a_hash_that_is_part_of_the_value() {
        assert_removes(
            "dependencies:\n- packs/a#b\n- packs/a\n",
            &["packs/a"],
            "dependencies:\n- packs/a#b\n",
        );
    }

    #[test]
    fn handles_a_comment_on_the_key_line() {
        assert_removes(
            "dependencies: # keep sorted\n- packs/a\n- packs/b\n",
            &["packs/b"],
            "dependencies: # keep sorted\n- packs/a\n",
        );
    }

    #[test]
    fn handles_blank_lines_inside_the_list() {
        assert_removes(
            "dependencies:\n- packs/a\n\n- packs/b\n\nlayer: utilities\n",
            &["packs/b"],
            "dependencies:\n- packs/a\n\nlayer: utilities\n",
        );
    }

    #[test]
    fn removes_the_comment_directly_above_a_removed_item() {
        assert_removes(
            "dependencies:\n# Needed for A.\n- packs/a\n# Needed for B.\n- packs/b\n",
            &["packs/a"],
            "dependencies:\n# Needed for B.\n- packs/b\n",
        );
    }

    #[test]
    fn keeps_a_comment_separated_from_a_removed_item_by_a_blank_line() {
        assert_removes(
            "dependencies:\n# Keep sorted.\n\n- packs/a\n- packs/b\n",
            &["packs/a"],
            "dependencies:\n# Keep sorted.\n\n- packs/b\n",
        );
    }

    #[test]
    fn keeps_a_comment_between_the_list_and_the_next_key() {
        assert_removes(
            "dependencies:\n- packs/a\n- packs/b\n# About the layer.\nlayer: utilities\n",
            &["packs/b"],
            "dependencies:\n- packs/a\n# About the layer.\nlayer: utilities\n",
        );
    }

    #[test]
    fn leaves_ignored_dependencies_and_nested_keys_alone() {
        assert_removes(
            "\
metadata:
  dependencies:
  - packs/a
dependencies:
- packs/a
- packs/b
ignored_dependencies:
# Deliberately ignored: a would cycle.
- packs/a
",
            &["packs/a"],
            "\
metadata:
  dependencies:
  - packs/a
dependencies:
- packs/b
ignored_dependencies:
# Deliberately ignored: a would cycle.
- packs/a
",
        );
    }

    #[test]
    fn preserves_crlf_line_endings() {
        assert_removes(
            "# Header.\r\ndependencies:\r\n- packs/a\r\n- packs/b\r\n",
            &["packs/b"],
            "# Header.\r\ndependencies:\r\n- packs/a\r\n",
        );
    }

    #[test]
    fn keeps_a_comment_while_any_item_it_covers_remains() {
        assert_removes(
            "dependencies:\n# Keep this list sorted.\n- packs/baz\n- packs/bar\n",
            &["packs/baz"],
            "dependencies:\n# Keep this list sorted.\n- packs/bar\n",
        );
        assert_removes(
            "dependencies:\n# Keep this list sorted.\n- packs/baz\n- packs/bar\n\n- packs/bop\n",
            &["packs/baz", "packs/bar"],
            "dependencies:\n\n- packs/bop\n",
        );
    }

    #[test]
    fn keeps_a_commented_out_entry() {
        assert_removes(
            "dependencies:\n- packs/bar\n# - packs/bop  # re-add after the migration\n- packs/baz\n",
            &["packs/baz"],
            "dependencies:\n- packs/bar\n# - packs/bop  # re-add after the migration\n",
        );
    }

    #[test]
    fn removes_the_whole_block_when_no_items_remain() {
        assert_removes(
            "enforce_dependencies: true\ndependencies: # keep sorted\n  - packs/a\n  # Keep sorted.\n\nenforce_privacy: true\n",
            &["packs/a"],
            "enforce_dependencies: true\nenforce_privacy: true\n",
        );
        assert_removes(
            "metadata:\n  notes: |\n    Some notes.\ndependencies:\n  # Needed for A.\n  - packs/a\n\n  # Needed for B.\n  - packs/b\nlayer: utilities\n",
            &["packs/a", "packs/b"],
            "metadata:\n  notes: |\n    Some notes.\nlayer: utilities\n",
        );
    }

    #[test]
    fn declines_layouts_it_cannot_edit() {
        assert_eq!(
            remove("dependencies: [packs/a, packs/b]\n", &["packs/b"]),
            None
        );
        assert_eq!(
            remove("dependencies:\n- \"packs/\\u0061\"\n", &["packs/a"]),
            None
        );
    }
}
