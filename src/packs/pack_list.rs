use crate::packs::pack::{write_pack_to_disk, Pack};

/// A top-level `package.yml` key whose value is a list of pack names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PackList {
    Dependencies,
    IgnoredDependencies,
    VisibleTo,
}

impl PackList {
    pub(crate) const ALL: [PackList; 3] = [
        PackList::Dependencies,
        PackList::IgnoredDependencies,
        PackList::VisibleTo,
    ];

    pub(crate) fn key(self) -> &'static str {
        match self {
            PackList::Dependencies => "dependencies",
            PackList::IgnoredDependencies => "ignored_dependencies",
            PackList::VisibleTo => "visible_to",
        }
    }

    pub(crate) fn contains(self, pack: &Pack, name: &str) -> bool {
        match self {
            PackList::Dependencies => pack.dependencies.contains(name),
            PackList::IgnoredDependencies => {
                pack.ignored_dependencies.contains(name)
            }
            PackList::VisibleTo => pack
                .visible_to
                .as_ref()
                .is_some_and(|visible_to| visible_to.contains(name)),
        }
    }

    fn remove(self, pack: &mut Pack, names: &[String]) {
        let keep = |name: &String| !names.contains(name);
        match self {
            PackList::Dependencies => pack.dependencies.retain(keep),
            PackList::IgnoredDependencies => {
                pack.ignored_dependencies.retain(keep)
            }
            PackList::VisibleTo => {
                if let Some(visible_to) = &mut pack.visible_to {
                    visible_to.retain(keep)
                }
            }
        }
    }
}

/// Edits `pack`'s `package.yml` in place where possible. Otherwise rewrites it,
/// with a warning, because rewriting loses its comments and key order.
pub(crate) fn remove_from_package_yml(
    pack: &Pack,
    lists: &[PackList],
    names: &[String],
) -> anyhow::Result<()> {
    let contents = std::fs::read_to_string(&pack.yml).map_err(|e| {
        anyhow::Error::new(e)
            .context(format!("Failed to read pack {:?}", pack.yml))
    })?;

    match remove_from_lists(&contents, lists, names) {
        Some(updated) if updated == contents => {}
        Some(updated) => std::fs::write(&pack.yml, updated).map_err(|e| {
            anyhow::Error::new(e)
                .context(format!("Failed to write pack to disk {:?}", pack.yml))
        })?,
        None => {
            eprintln!(
                "Warning: could not edit the {} in {} in place, so the file \
                 was rewritten and its comments were not preserved.",
                describe(lists),
                pack.relative_yml().display()
            );
            let mut updated_pack = pack.clone();
            for list in lists {
                list.remove(&mut updated_pack, names);
            }
            write_pack_to_disk(&updated_pack)?;
        }
    }

    Ok(())
}

fn describe(lists: &[PackList]) -> String {
    let keys: Vec<&str> = lists.iter().map(|list| list.key()).collect();
    match keys.split_last() {
        Some((last, [])) => format!("{} list", last),
        Some((last, rest)) => format!("{} and {} lists", rest.join(", "), last),
        None => String::from("lists"),
    }
}

/// Returns `contents` with `names` removed from each of `lists`, edited as
/// text so comments, key order and line endings survive.
///
/// Returns `None` when the edited text does not parse back to the original
/// pack minus exactly `names`, which happens when a list uses a layout the
/// line matcher does not recognise (flow style, multi-line scalars, ...).
fn remove_from_lists(
    contents: &str,
    lists: &[PackList],
    names: &[String],
) -> Option<String> {
    let edited = lists.iter().fold(contents.to_owned(), |edited, list| {
        remove_list_lines(&edited, *list, names)
    });
    is_exact_removal(contents, &edited, lists, names).then_some(edited)
}

fn is_exact_removal(
    original: &str,
    edited: &str,
    lists: &[PackList],
    names: &[String],
) -> bool {
    let (Ok(mut expected), Ok(actual)) = (
        yaml_serde::from_str::<Pack>(original),
        yaml_serde::from_str::<Pack>(edited),
    ) else {
        return false;
    };
    for list in lists {
        list.remove(&mut expected, names);
    }
    expected == actual
}

/// Drops each matching item, along with any comment lines directly above it.
/// Other comments, blank lines and blocks are kept verbatim. If no items are
/// left, the key goes too, as serializing the pack would do. `visible_to` is
/// the exception: serializing writes it as `visible_to: []`, so that is what
/// the key becomes.
fn remove_list_lines(
    contents: &str,
    list: PackList,
    names: &[String],
) -> String {
    let mut out: Vec<&str> = Vec::new();
    // Comments seen inside the block, held until we know whether the item
    // below them is kept.
    let mut pending_comments: Vec<&str> = Vec::new();
    let mut key_index = None;
    let mut in_block = false;
    let mut kept_items = 0;

    for line in contents.split_inclusive('\n') {
        if in_block {
            let trimmed = line.trim_start();
            if let Some(item) = trimmed.strip_prefix("- ") {
                if names.iter().any(|name| name == item_value(item)) {
                    pending_comments.clear();
                } else {
                    out.append(&mut pending_comments);
                    out.push(line);
                    kept_items += 1;
                }
                continue;
            }
            if trimmed.starts_with('#') {
                pending_comments.push(line);
                continue;
            }
            if trimmed.is_empty() {
                out.append(&mut pending_comments);
                out.push(line);
                continue;
            }
            // A comment between the last item and the next key belongs to
            // the key.
            out.append(&mut pending_comments);
            in_block = false;
        } else if key_index.is_none() && is_key(line, list.key()) {
            key_index = Some(out.len());
            in_block = true;
        }
        out.push(line);
    }
    out.append(&mut pending_comments);

    match key_index.filter(|_| kept_items == 0) {
        None => out.concat(),
        Some(index) if list == PackList::VisibleTo => {
            // `is_key` matched, so the line starts with `visible_to:`.
            let after_key = &out[index][list.key().len() + 1..];
            format!(
                "{}{}: []{}{}",
                out[..index].concat(),
                list.key(),
                after_key,
                out[index + 1..].concat()
            )
        }
        Some(index) => {
            out.remove(index);
            out.concat()
        }
    }
}

/// Matches the top-level key only: a nested key of the same name is indented.
fn is_key(line: &str, key: &str) -> bool {
    strip_comment(line).trim_end().strip_suffix(':') == Some(key)
}

/// The scalar value of a block list item, without quotes or a trailing
/// comment. Escapes inside quotes are not decoded; the parse check in
/// `remove_from_lists` catches any item that this misreads.
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
    use super::{describe, remove_from_lists, PackList};
    use pretty_assertions::assert_eq;

    fn remove_from(
        contents: &str,
        lists: &[PackList],
        names: &[&str],
    ) -> Option<String> {
        let names: Vec<String> = names.iter().map(|n| n.to_string()).collect();
        remove_from_lists(contents, lists, &names)
    }

    fn remove(contents: &str, names: &[&str]) -> Option<String> {
        remove_from(contents, &[PackList::Dependencies], names)
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
            "dependencies:\n- packs/a\n\n\nlayer: utilities\n",
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
    fn declines_layouts_it_cannot_edit() {
        assert_eq!(
            remove("dependencies: [packs/a, packs/b]\n", &["packs/b"]),
            None
        );
        assert_eq!(
            remove("dependencies:\n- \"packs/\\u0061\"\n", &["packs/a"]),
            None
        );
        assert_eq!(
            remove_from(
                "visible_to: [packs/a, packs/b]\n",
                &[PackList::VisibleTo],
                &["packs/a"]
            ),
            None
        );
    }

    #[test]
    fn removes_a_name_from_every_list_at_once() {
        let before = "\
# Header.
enforce_visibility: true
dependencies:
- packs/a
- packs/b
ignored_dependencies:
# Deliberately ignored: a would cycle.
- packs/a
visible_to:
- packs/a
- packs/c
metadata:
  visible_to:
  - packs/a
";
        let after = "\
# Header.
enforce_visibility: true
dependencies:
- packs/b
visible_to:
- packs/c
metadata:
  visible_to:
  - packs/a
";
        assert_eq!(
            remove_from(before, &PackList::ALL, &["packs/a"]).as_deref(),
            Some(after)
        );
    }

    #[test]
    fn empties_visible_to_instead_of_removing_it() {
        let visible_to = &[PackList::VisibleTo];
        assert_eq!(
            remove_from(
                "visible_to:\n- packs/a\nlayer: utilities\n",
                visible_to,
                &["packs/a"]
            )
            .as_deref(),
            Some("visible_to: []\nlayer: utilities\n")
        );
        assert_eq!(
            remove_from(
                "visible_to: # Only a.\r\n- packs/a\r\n",
                visible_to,
                &["packs/a"]
            )
            .as_deref(),
            Some("visible_to: [] # Only a.\r\n")
        );
    }

    #[test]
    fn describes_the_lists_in_the_fallback_warning() {
        assert_eq!(describe(&[PackList::Dependencies]), "dependencies list");
        assert_eq!(
            describe(&[PackList::Dependencies, PackList::VisibleTo]),
            "dependencies and visible_to lists"
        );
        assert_eq!(
            describe(&PackList::ALL),
            "dependencies, ignored_dependencies and visible_to lists"
        );
    }
}
