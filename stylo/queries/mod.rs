/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Code shared between [media queries][mq] and [container queries][cq].
//!
//! [mq]: https://drafts.csswg.org/mediaqueries/
//! [cq]: https://drafts.csswg.org/css-contain-3/#container-rule

pub mod condition;

#[macro_use]
pub mod feature;
pub mod feature_expression;
pub mod values;

pub use self::condition::QueryCondition;
pub use self::feature::FeatureFlags;
pub use self::feature_expression::{FeatureType, QueryFeatureExpression, QueryStyleRange};

/// Whether `css` holds a `random()` or `random-item()` function at any depth. Media and
/// container queries do not accept them (https://github.com/w3c/csswg-drafts/issues/10982;
/// Snowlark patch 108).
pub(crate) fn contains_random_function(css: &str) -> bool {
    fn scan(input: &mut cssparser::Parser) -> bool {
        loop {
            let (random, block) = match input.next() {
                Err(_) => return false,
                Ok(cssparser::Token::Function(name)) => (
                    name.eq_ignore_ascii_case("random") || name.eq_ignore_ascii_case("random-item"),
                    true,
                ),
                Ok(
                    cssparser::Token::ParenthesisBlock
                    | cssparser::Token::SquareBracketBlock
                    | cssparser::Token::CurlyBracketBlock,
                ) => (false, true),
                Ok(_) => (false, false),
            };
            if random {
                return true;
            }
            if block
                && input
                    .parse_nested_block(|i| {
                        Ok::<_, style_traits::ParseError>(scan(i))
                    })
                    .unwrap_or(false)
            {
                return true;
            }
        }
    }
    css.contains("random") && scan(&mut cssparser::Parser::new(css))
}