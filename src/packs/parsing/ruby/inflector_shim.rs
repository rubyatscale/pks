use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;
use ruby_inflector::case::{
    to_case_camel_like, to_class_case as to_class_case_original, CamelOptions,
};

// This is a list of plural to singular words that are not handled by the inflector
// The plural words are
const CLASS_CASE_TO_SINGULAR: [(&str, &str); 4] = [
    ("Censuse", "Census"),
    ("Leafe", "Leave"),
    ("Lefe", "Leave"),
    ("Daum", "Datum"),
];

// `camelize` and `to_class_case` run once per file while the Zeitwerk constant
// map is built, so on a large codebase these are on the order of 50k calls per
// invocation. Compiling a regex is expensive (it builds a DFA), so every pattern
// used here is compiled once for the life of the process rather than per call.
static STATUSE_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("Statuse$").unwrap());
static STATU_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("Statu$").unwrap());
static STATUSS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("Statuss").unwrap());
static SINGULARIZE: LazyLock<[(Regex, &str); CLASS_CASE_TO_SINGULAR.len()]> =
    LazyLock::new(|| {
        CLASS_CASE_TO_SINGULAR
            .map(|(plural, singular)| (Regex::new(plural).unwrap(), singular))
    });
static LEADING_LOWERCASE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^[a-z\\d]*").unwrap());
static UNDERSCORE_OR_SLASH_WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("(?:_|(/))([a-z\\d]*)").unwrap());

// See https://github.com/whatisinternet/Inflector/pull/87
// Note that as of the PR that adds this comment, we are now using https://github.com/alexevanczuk/ruby_inflector,
// so that we have an easier time making this inflector more specific to ruby applications (for now)
pub fn to_class_case(
    s: &str,
    should_singularize: bool,
    acronyms: &HashSet<String>,
) -> String {
    let options = CamelOptions {
        new_word: true,
        last_char: ' ',
        first_word: false,
        injectable_char: ' ',
        has_seperator: false,
        inverted: false,
    };

    let mut class_name = if should_singularize {
        to_class_case_original(s, acronyms)
    } else {
        to_case_camel_like(s, options, acronyms)
    };

    if class_name.contains("Statu") {
        class_name = STATUSE_SUFFIX
            .replace_all(&class_name, "Status")
            .to_string();
        class_name =
            STATU_SUFFIX.replace_all(&class_name, "Status").to_string();

        // NOTE: the result of this replacement has always been discarded, so
        // "Statuss" is left alone (see the ("statuss", false, "Statuss") case in
        // the tests below). Preserved verbatim here: this commit is only meant to
        // stop recompiling regexes, not to change what the inflector produces.
        STATUSS.replace_all(&class_name, "Status").to_string();
    }

    SINGULARIZE.iter().for_each(|(re, singular)| {
        // `contains` on the original literal is a cheap pre-filter that avoids
        // running the regex at all for the common case of no match.
        if class_name.contains(re.as_str()) {
            class_name = re.replace_all(&class_name, *singular).to_string();
        }
    });

    class_name
}

pub fn camelize(s: &str, acronyms: &HashSet<String>) -> String {
    // Meant to emulate https://github.com/rails/rails/blob/e88857bbb9d4e1dd64555c34541301870de4a45b/activesupport/lib/active_support/inflector/methods.rb#L69
    //
    // def camelize(term, uppercase_first_letter = true)
    //   string = term.to_s
    //   # String#camelize takes a symbol (:upper or :lower), so here we also support :lower to keep the methods consistent.
    //   if !uppercase_first_letter || uppercase_first_letter == :lower
    //     string = string.sub(inflections.acronyms_camelize_regex) { |match| match.downcase! || match }
    //   else
    //     string = string.sub(/^[a-z\d]*/) { |match| inflections.acronyms[match] || match.capitalize! || match }
    //   end
    //   string.gsub!(/(?:_|(\/))([a-z\d]*)/i) do
    //     word = $2
    //     substituted = inflections.acronyms[word] || word.capitalize! || word
    //     $1 ? "::#{substituted}" : substituted
    //   end
    //   string
    // end

    let lowercase_acronyms_to_originals = acronyms
        .iter()
        .map(|acronym| (acronym.to_lowercase(), acronym))
        .collect::<HashMap<String, &String>>();

    let mut new_string = s.to_string();
    // Replace the beginning of the word, matched with lowercase letters, with either a matching inflection or a capitalized version of the word
    new_string = LEADING_LOWERCASE
        .replace(&new_string, |caps: &regex::Captures| {
            let word = caps.get(0).unwrap().as_str();
            if lowercase_acronyms_to_originals.contains_key(word) {
                lowercase_acronyms_to_originals[word].to_string()
            } else {
                capitalize(word)
            }
        })
        .to_mut()
        .to_string();

    new_string = UNDERSCORE_OR_SLASH_WORD
        .replace_all(&new_string, |caps: &regex::Captures| {
            let matched_slash = caps.get(1);
            let word = caps.get(2).unwrap().as_str();
            let capitalized_word =
                if lowercase_acronyms_to_originals.contains_key(word) {
                    lowercase_acronyms_to_originals[word].to_string()
                } else {
                    capitalize(word)
                };

            if matched_slash.is_some() {
                format!("::{}", capitalized_word)
            } else {
                capitalized_word
            }
        })
        .to_mut()
        .to_string();

    new_string
}

/// Capitalizes the first character in s.
fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

// Add tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trivial() {
        let actual = to_class_case("my_string", false, &HashSet::new());
        let expected = "MyString";
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_digits() {
        let actual =
            to_class_case("my_string_401k_thing", false, &HashSet::new());
        let expected = "MyString401kThing";
        assert_eq!(expected, actual);
    }

    #[test]
    fn fn_test_camelizing_case_retained() {
        let mut acronyms = HashSet::new();
        acronyms.insert(String::from("FacTory"));

        let actual = camelize("my_factory", &acronyms);
        let expected = "MyFacTory";
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_to_class_case() {
        let tests = vec![
            ("my_string", false, "MyString"),
            ("censuses", true, "Census"),
            ("lefe", true, "Leave"),
            ("leaves", false, "Leaves"),
            ("daum", true, "Datum"),
            ("statuss", false, "Statuss"),
            ("statuses", true, "Status"),
            ("censuse", true, "Census"),
        ];

        for (input, should_singularize, expected) in tests {
            let actual =
                to_class_case(input, should_singularize, &HashSet::new());
            assert_eq!(
                expected, actual,
                "Failed for input: {}, and singularize: {}",
                input, should_singularize
            );
        }
    }
}
