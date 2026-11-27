/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The `if()` arbitrary substitution function (Snowlark patch 108).
//! https://drafts.csswg.org/css-values-5/#if-notation

use super::{
    ArbitraryDeclarationValue, ReferenceFlags, Substitution, SubstitutionParserContext,
    SubstitutionReplacementContext,
};
use std::borrow::Cow;
use crate::computed_value_flags::ComputedValueFlags;
use crate::custom_properties::{Name, VariableValue};
use crate::derives::*;
use crate::parser::ParserContext;
use crate::queries::condition::{QueryCondition, StyleQuery, StyleQueryTarget};
use crate::queries::{FeatureType, QueryFeatureExpression};
use crate::stylesheets::supports_rule::{Declaration, SupportsCondition};
use crate::stylesheets::{CssRuleType, Origin, CustomMediaEvaluator};
use crate::substitution::AttrTaint;
use cssparser::{Delimiter, Parser, Token, match_ignore_ascii_case};
use selectors::kleene_value::KleeneValue;
use style_traits::{ParseError, ParsingMode, StyleParseErrorKind};

/// One `<if-test>`, or a `<boolean-expr>` of them.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToShmem)]
pub enum IfCondition {
    /// `not <group>`.
    Not(Box<IfCondition>),
    /// `<group> and <group> ...`.
    And(Box<[IfCondition]>),
    /// `<group> or <group> ...`.
    Or(Box<[IfCondition]>),
    /// `style(<style-query>)`, tested against the element's own custom properties.
    Style(StyleQuery),
    /// `media(<media-feature> | <media-condition>)`.
    Media(QueryCondition),
    /// `supports(...)`, evaluated when parsed.
    Supports(bool),
    /// A `<general-enclosed>`: unknown, so false.
    Unknown,
}

impl IfCondition {
    fn error() -> ParseError {
        ParseError::custom(StyleParseErrorKind::UnspecifiedError)
    }

    /// `<boolean-expr[ <if-test> ]>`.
    fn parse(
        context: &ParserContext,
        input: &mut Parser,
    ) -> Result<Self, ParseError> {
        if input.try_parse(|i| i.expect_ident_matching("not")).is_ok() {
            return Ok(Self::Not(Box::new(Self::parse_group(context, input)?)));
        }
        let first = Self::parse_group(context, input)?;
        let and = if input.try_parse(|i| i.expect_ident_matching("and")).is_ok() {
            true
        } else if input.try_parse(|i| i.expect_ident_matching("or")).is_ok() {
            false
        } else {
            return Ok(first);
        };
        let keyword = if and { "and" } else { "or" };
        let mut list = vec![first, Self::parse_group(context, input)?];
        while input.try_parse(|i| i.expect_ident_matching(keyword)).is_ok() {
            list.push(Self::parse_group(context, input)?);
        }
        let list = list.into_boxed_slice();
        Ok(if and { Self::And(list) } else { Self::Or(list) })
    }

    /// `<boolean-expr-group> = <if-test> | ( <boolean-expr> ) | <general-enclosed>`.
    fn parse_group(
        context: &ParserContext,
        input: &mut Parser,
    ) -> Result<Self, ParseError> {
        input.skip_whitespace();
        let token = input.next()?.clone();
        match token {
            Token::ParenthesisBlock => {
                if let Ok(c) = input.try_parse(|i| {
                    i.parse_nested_block(|i| Self::parse(context, i))
                }) {
                    return Ok(c);
                }
                input.parse_nested_block(|i| i.expect_no_error_token().map_err(Into::into))?;
                Ok(Self::Unknown)
            },
            // A substitution function is replaced before the condition is parsed.
            Token::Function(ref name)
                if super::SubstitutionFunctionKind::from_ident(name).is_ok() =>
            {
                Err(Self::error())
            },
            Token::Function(name) => {
                let test = input.try_parse(|i| {
                    i.parse_nested_block(|i| {
                        match_ignore_ascii_case! { &name,
                            "style" => Ok(Self::Style(StyleQuery::parse_for_if(context, i)?)),
                            "media" => Self::parse_media(context, i),
                            "supports" => Self::parse_supports(context, i),
                            _ => Err(Self::error()),
                        }
                    })
                });
                if let Ok(test) = test {
                    return Ok(test);
                }
                input.parse_nested_block(|i| i.expect_no_error_token().map_err(Into::into))?;
                Ok(Self::Unknown)
            },
            _ => Err(Self::error()),
        }
    }

    fn parse_media(
        context: &ParserContext,
        input: &mut Parser,
    ) -> Result<Self, ParseError> {
        if let Ok(feature) = input.try_parse(|i| {
            QueryFeatureExpression::parse_in_parenthesis_block(context, i, FeatureType::Media)
        }) {
            return Ok(Self::Media(QueryCondition::Feature(feature)));
        }
        Ok(Self::Media(QueryCondition::parse(
            context,
            input,
            FeatureType::Media,
        )?))
    }

    fn parse_supports(
        context: &ParserContext,
        input: &mut Parser,
    ) -> Result<Self, ParseError> {
        let mut eval_context = ParserContext::new(
            Origin::Author,
            context.url_data,
            Some(CssRuleType::Style),
            ParsingMode::DEFAULT,
            context.quirks_mode,
            Default::default(),
            None,
            None,
            AttrTaint::default(),
        );
        if let Ok(declaration) = input.try_parse(|i| Declaration::parse(i)) {
            return Ok(Self::Supports(declaration.eval(&mut eval_context)));
        }
        let condition = SupportsCondition::parse(input)?;
        Ok(Self::Supports(condition.eval(&mut eval_context)))
    }

    /// Calls `f` with the name of every custom property a `style()` test in it names.
    pub fn for_each_custom_property(&self, f: &mut impl FnMut(&Name)) {
        match self {
            Self::Not(c) => c.for_each_custom_property(f),
            Self::And(list) | Self::Or(list) => {
                list.iter().for_each(|c| c.for_each_custom_property(f))
            },
            Self::Style(query) => query.for_each_custom_property(f),
            Self::Media(..) | Self::Supports(..) | Self::Unknown => {},
        }
    }

    /// Calls `f` with each value in a `style()` test that holds substitution functions.
    pub fn for_each_value<'a>(&'a self, f: &mut impl FnMut(&'a VariableValue)) {
        match self {
            Self::Not(c) => c.for_each_value(f),
            Self::And(list) | Self::Or(list) => list.iter().for_each(|c| c.for_each_value(f)),
            Self::Style(query) => query.for_each_value(f),
            Self::Media(..) | Self::Supports(..) | Self::Unknown => {},
        }
    }

    /// Whether the condition holds for the element.
    pub fn holds(&self, context: &mut SubstitutionReplacementContext) -> bool {
        self.evaluate(context) == KleeneValue::True
    }

    fn evaluate(&self, context: &mut SubstitutionReplacementContext) -> KleeneValue {
        match self {
            Self::Not(c) => !c.evaluate(context),
            Self::And(list) => KleeneValue::any_false(list.iter(), |c| c.evaluate(context)),
            Self::Or(list) => KleeneValue::any(list.iter(), |c| c.evaluate(context)),
            Self::Style(query) => query.matches_for(
                context.computed_context,
                context.attribute_tracker,
                StyleQueryTarget::Element(context.substitution_functions, &context.seen),
            ),
            Self::Media(condition) => {
                // A media() test changes with the viewport, as viewport units do.
                context
                    .computed_context
                    .builder
                    .add_flags(ComputedValueFlags::USES_VIEWPORT_UNITS);
                condition.matches(
                    context.computed_context,
                    &mut CustomMediaEvaluator::none(),
                    context.attribute_tracker,
                )
            },
            Self::Supports(result) => KleeneValue::from(*result),
            Self::Unknown => KleeneValue::Unknown,
        }
    }

    /// Whether testing the condition reads attr()-tainted values: a tested custom property
    /// holding one, or a query value with an attr() in it.
    fn reads_attr_taint(&self, context: &SubstitutionReplacementContext) -> bool {
        let mut tainted = false;
        self.for_each_custom_property(&mut |name| {
            let registration = context.stylist.get_custom_property_registration(name);
            tainted |= context
                .substitution_functions
                .get_var(registration, name)
                .is_some_and(|v| v.attr_tainted);
        });
        self.for_each_value(&mut |value| {
            tainted |= value.references.flags.intersects(ReferenceFlags::ATTR);
        });
        tainted
    }

    /// Parses a whole condition, `else` giving `None`; one that is not an `<if-condition>`
    /// never holds.
    fn parse_entirely(context: &ParserContext, css: &str) -> Option<Self> {
        let mut input = Parser::new(css);
        if input
            .try_parse(|i| -> Result<(), ParseError> {
                i.expect_ident_matching("else")?;
                Ok(i.expect_exhausted()?)
            })
            .is_ok()
        {
            return None;
        }
        Some(
            input
                .parse_entirely(|i| Self::parse(context, i))
                .unwrap_or(Self::Unknown),
        )
    }
}

/// An `<if-branch>`'s condition.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToShmem)]
pub enum IfBranchCondition {
    /// A condition without substitution functions, parsed (`None`: `else`).
    Parsed(Option<IfCondition>),
    /// A condition with substitution functions, parsed after they are substituted.
    Unparsed(ArbitraryDeclarationValue),
}

/// One `<if-branch>`: a condition and its value.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToShmem)]
pub struct IfBranch {
    /// The condition.
    pub condition: IfBranchCondition,
    /// The value, which may be empty.
    pub value: ArbitraryDeclarationValue,
}

impl IfBranch {
    /// The branch's condition, its substitution functions replaced (`None`: `else`), and
    /// whether that involved attr()-tainted values.
    pub fn resolve_condition<'a, 'c>(
        &'a self,
        source: &'a VariableValue,
        context: &mut SubstitutionReplacementContext<'c, '_, '_, '_>,
    ) -> (Option<Cow<'a, IfCondition>>, bool)
    where
        'a: 'c,
    {
        match self.condition {
            IfBranchCondition::Parsed(ref c) => (c.as_ref().map(Cow::Borrowed), false),
            IfBranchCondition::Unparsed(ref arg) => {
                let Ok(substitution) = context.substitute_declaration_value(arg, source) else {
                    return (Some(Cow::Owned(IfCondition::Unknown)), false);
                };
                let parser_context = context.parser_context(&source.url_data);
                (
                    IfCondition::parse_entirely(&parser_context, &substitution.css).map(Cow::Owned),
                    substitution.attr_tainted,
                )
            },
        }
    }
}

/// Data specific to an `if()` function.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToShmem)]
pub struct IfFunction {
    /// The branches, in order.
    pub branches: Box<[IfBranch]>,
}

impl IfFunction {
    /// Parses `if( [ <if-branch> ; ]* <if-branch> ;? )`. As an arbitrary substitution function,
    /// its branches are `<declaration-value> : <declaration-value>?` here; a condition is parsed
    /// as `<if-condition>` once its substitution functions are replaced.
    pub fn parse(
        input: &mut Parser,
        context: &mut SubstitutionParserContext,
    ) -> Result<Self, ParseError> {
        let mut branches = vec![];
        loop {
            input.skip_whitespace();
            // A condition that parses as it is, before its colon, keeps the var() in its
            // style() tests for them to substitute; any other is parsed after substitution.
            let parser_context = context.parser_context;
            let direct = input.try_parse(|i| -> Result<Option<IfCondition>, ParseError> {
                let condition = if i.try_parse(|i| i.expect_ident_matching("else")).is_ok() {
                    None
                } else {
                    Some(IfCondition::parse(parser_context, i)?)
                };
                let before_colon = i.state();
                i.expect_colon()?;
                i.reset(&before_colon);
                Ok(condition)
            });
            let condition = match direct {
                Ok(condition) => IfBranchCondition::Parsed(condition),
                Err(_) => {
                    let (condition, css) =
                        ArbitraryDeclarationValue::parse_if_condition(input, context)?;
                    if condition.is_empty() {
                        return Err(IfCondition::error());
                    }
                    if condition.references.has_references() {
                        IfBranchCondition::Unparsed(condition)
                    } else {
                        IfBranchCondition::Parsed(IfCondition::parse_entirely(
                            context.parser_context,
                            css,
                        ))
                    }
                },
            };
            input.expect_colon()?;
            input.skip_whitespace();
            let value = ArbitraryDeclarationValue::parse_internal(
                input,
                context,
                Delimiter::Semicolon,
            )?;
            branches.push(IfBranch { condition, value });
            if input.is_exhausted() {
                break;
            }
            input.expect_semicolon()?;
            input.skip_whitespace();
            if input.is_exhausted() {
                break;
            }
        }
        Ok(Self {
            branches: branches.into_boxed_slice(),
        })
    }

    /// Replaces the `if()` function with the value of its first branch whose condition holds;
    /// with none, the function is invalid at computed-value time. The value is attr()-tainted
    /// when testing the conditions read attr()-tainted values.
    pub fn replace<'a>(
        &'a self,
        source: &'a VariableValue,
        context: &mut SubstitutionReplacementContext<'a, '_, '_, '_>,
    ) -> Result<Substitution<'a>, ()> {
        let mut tainted = false;
        for branch in self.branches.iter() {
            let (condition, condition_tainted) = branch.resolve_condition(source, context);
            tainted |= condition_tainted;
            let holds = match condition {
                None => true,
                Some(condition) => {
                    tainted |= condition.reads_attr_taint(context);
                    condition.holds(context)
                },
            };
            if holds {
                let mut substitution = context.substitute_declaration_value(&branch.value, source)?;
                substitution.attr_tainted |= tainted;
                return Ok(substitution);
            }
        }
        Err(())
    }

    /// Calls `f` for each arbitrary declaration value nested in the function.
    pub fn for_each_unparsed_value<F>(&self, mut f: F)
    where
        F: FnMut(&ArbitraryDeclarationValue),
    {
        for branch in self.branches.iter() {
            if let IfBranchCondition::Unparsed(ref condition) = branch.condition {
                f(condition);
            }
            f(&branch.value);
        }
    }
}
