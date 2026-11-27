/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Snowlark patch (SNOWLARK-PATCHES.md 107): computed `calc-size()` (CSS Sizing 4 §7) and the
//! interpolation of the sizing properties, which depends on the element's `interpolate-size`
//! (§7.3).

use super::calc::ComputedLeaf;
use super::length_percentage::{CalcNode, LengthPercentage, NonNegativeLengthPercentage};
use super::{MaxSize, Size};
use crate::derives::*;
use crate::values::animated::{Animate, Context as AnimatedContext, Procedure, ToAnimatedValue, ToAnimatedZero};
use crate::values::distance::{ComputeSquaredDistance, SquaredDistance};
use crate::values::generics::calc::CalcNodeLeaf;
use crate::values::generics::length::{
    GenericCalcSizeBasis, GenericCalcSizeFunction, GenericMaxSize, GenericSize,
};
use crate::values::generics::NonNegative;
use crate::values::specified::box_::InterpolateSize;
use style_traits::values::specified::AllowedNumericType;

/// A computed `calc-size()` function.
pub type CalcSizeFunction = GenericCalcSizeFunction<NonNegativeLengthPercentage>;
/// A computed `calc-size()` basis.
pub type CalcSizeBasis = GenericCalcSizeBasis<NonNegativeLengthPercentage>;

fn has_percentage(node: &CalcNode) -> bool {
    let mut found = false;
    let mut copy = node.clone();
    copy.visit_depth_first(|n| {
        if matches!(n, CalcNode::Leaf(ComputedLeaf::Percentage(..))) {
            found = true;
        }
    });
    found
}

/// The most nodes a calculation may have after a substitution (§7.2.1's "UA-defined limit"):
/// each nested basis that refers to `size` more than once multiplies the tree.
const SUBSTITUTION_LIMIT: usize = 10_000;

/// The nodes of a tree, and how many of them are `size`.
fn count(node: &CalcNode) -> (usize, usize) {
    let (mut nodes, mut sizes) = (0, 0);
    let mut copy = node.clone();
    copy.visit_depth_first(|n| {
        nodes += 1;
        if matches!(n, CalcNode::Leaf(ComputedLeaf::Size)) {
            sizes += 1;
        }
    });
    (nodes, sizes)
}

/// Replaces every `size` in `calc` with `with` (§7.2.1, "substitute"). A tree needs no
/// parentheses: the replacement is a subtree. Fails past the limit.
fn substitute(calc: &mut CalcNode, with: &CalcNode) -> Option<()> {
    let (nodes, sizes) = count(calc);
    let (with_nodes, _) = count(with);
    if nodes + sizes * with_nodes > SUBSTITUTION_LIMIT {
        return None;
    }
    calc.map_node(|n| {
        Ok(match n {
            CalcNode::Leaf(ComputedLeaf::Size) => Some(with.clone()),
            _ => None,
        })
    })
    .ok()
}

/// §7.2.1 "de-percentify": every percentage `p` becomes `(size * p)`.
fn depercentify(node: &mut CalcNode) {
    let _ = node.map_node(|n| {
        Ok(match n {
            CalcNode::Leaf(ComputedLeaf::Percentage(p)) => Some(CalcNode::Product(
                vec![
                    CalcNode::Leaf(ComputedLeaf::new_number(p.get())),
                    CalcNode::Leaf(ComputedLeaf::Size),
                ]
                .into(),
            )),
            _ => None,
        })
    });
}

impl CalcSizeFunction {
    /// `calc-size(<keyword>, size)`.
    pub fn keyword(basis: CalcSizeBasis) -> Self {
        Self {
            basis,
            calc: NonNegative(LengthPercentage::new_calc(
                CalcNode::Leaf(ComputedLeaf::Size),
                AllowedNumericType::NonNegative,
            )),
        }
    }

    /// The function with its lengths unzoomed, as lengths are in animated values.
    fn unzoomed(self, context: &AnimatedContext) -> Self {
        Self {
            basis: match self.basis {
                GenericCalcSizeBasis::LengthPercentage(lp) => {
                    GenericCalcSizeBasis::LengthPercentage(lp.to_animated_value(context))
                },
                GenericCalcSizeBasis::CalcSize(inner) => {
                    GenericCalcSizeBasis::CalcSize(Box::new(inner.unzoomed(context)))
                },
                keyword => keyword,
            },
            calc: self.calc.to_animated_value(context),
        }
    }

    /// `calc-size(any, <length-percentage>)`.
    pub fn any(lp: &NonNegativeLengthPercentage) -> Self {
        Self {
            basis: CalcSizeBasis::Any,
            calc: lp.clone(),
        }
    }

    /// §7.2.1, "prepare a calc-size() for interpolation": the basis becomes a keyword, `any`
    /// or `100%`, and the calculation the one tree that refers to it. `None` when a
    /// substitution passes the limit: such a value does not interpolate.
    pub fn prepare(&self) -> Option<(CalcSizeBasis, CalcNode)> {
        let mut calc = self.calc.0.to_calc_node();
        let basis = match &self.basis {
            GenericCalcSizeBasis::CalcSize(inner) => {
                let (basis, inner_calc) = inner.prepare()?;
                substitute(&mut calc, &inner_calc)?;
                basis
            },
            GenericCalcSizeBasis::LengthPercentage(lp) => {
                let mut b = lp.0.to_calc_node();
                if has_percentage(&b) {
                    depercentify(&mut b);
                    substitute(&mut calc, &b)?;
                    CalcSizeBasis::LengthPercentage(NonNegative(LengthPercentage::hundred_percent()))
                } else {
                    substitute(&mut calc, &b)?;
                    CalcSizeBasis::Any
                }
            },
            keyword => keyword.clone(),
        };
        calc.simplify_and_sort();
        Some((basis, calc))
    }

    /// §7.2 "interpolating calc-size()": the bases must be the same, or one `any`.
    fn animate_with(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        let (a_basis, a) = self.prepare().ok_or(())?;
        let (b_basis, b) = other.prepare().ok_or(())?;
        let basis = if a_basis == b_basis || b_basis == CalcSizeBasis::Any {
            a_basis
        } else if a_basis == CalcSizeBasis::Any {
            b_basis
        } else {
            return Err(());
        };
        let a = LengthPercentage::new_calc(a, AllowedNumericType::All);
        let b = LengthPercentage::new_calc(b, AllowedNumericType::All);
        let calc = a.animate(&b, procedure)?.to_calc_node();
        // A result linear in `size` is written `rest + k * size`, as a calculation is
        // simplified (CSS Values 4 §10.10.1 combines like terms; the products the
        // interpolation makes of `size` are not leaves it would combine). Only where `size` is:
        // two `any` bases make none.
        let split = if has_size(&calc) {
            split_size(&calc)
        } else {
            None
        };
        let calc = match split {
            Some((k, rest)) => CalcNode::Sum(
                vec![
                    rest,
                    CalcNode::Product(
                        vec![
                            CalcNode::Leaf(ComputedLeaf::new_number(k)),
                            CalcNode::Leaf(ComputedLeaf::Size),
                        ]
                        .into(),
                    ),
                ]
                .into(),
            ),
            None => calc,
        };
        Ok(Self {
            basis,
            calc: NonNegative(LengthPercentage::new_calc(calc, AllowedNumericType::NonNegative)),
        })
    }
}

fn has_size(node: &CalcNode) -> bool {
    let mut found = false;
    let mut copy = node.clone();
    copy.visit_depth_first(|n| {
        if matches!(n, CalcNode::Leaf(ComputedLeaf::Size)) {
            found = true;
        }
    });
    found
}

/// `node` as `k * size + rest`, when it is linear in `size`.
fn split_size(node: &CalcNode) -> Option<(f32, CalcNode)> {
    let zero = || CalcNode::Leaf(ComputedLeaf::Length(super::Length::new(0.)));
    Some(match node {
        CalcNode::Leaf(ComputedLeaf::Size) => (1., zero()),
        CalcNode::Negate(x) => {
            let (k, rest) = split_size(x)?;
            (-k, CalcNode::Negate(Box::new(rest)))
        },
        CalcNode::Sum(items) => {
            let mut k = 0.;
            let mut rest = Vec::with_capacity(items.len());
            for item in items.iter() {
                let (ki, r) = split_size(item)?;
                k += ki;
                rest.push(r);
            }
            (k, CalcNode::Sum(rest.into()))
        },
        CalcNode::Product(items) if has_size(node) => {
            let mut factor = 1.;
            let mut term = None;
            for item in items.iter() {
                match item {
                    CalcNode::Leaf(ComputedLeaf::Number(n)) => factor *= n,
                    _ if term.is_none() => term = Some(split_size(item)?),
                    _ => return None,
                }
            }
            let (k, rest) = term?;
            (
                k * factor,
                CalcNode::Product(vec![rest, CalcNode::Leaf(ComputedLeaf::new_number(factor))].into()),
            )
        },
        other if !has_size(other) => (0., other.clone()),
        _ => return None,
    })
}

/// A sizing property's value as a `calc-size()` for interpolation (§7.2): a
/// `<length-percentage>` is `calc-size(any, value)`, and a `<size-keyword>` is
/// `calc-size(value, size)` when `keywords` allows it. `None` for values that never interpolate
/// this way (`none`, `fit-content()`, anchor functions).
pub trait AsCalcSize: Sized {
    /// The value as a `calc-size()`.
    fn as_calc_size(&self, keywords: bool) -> Option<CalcSizeFunction>;
    /// Whether the value is a `calc-size()`.
    fn is_calc_size(&self) -> bool;
    /// The value for an interpolated `calc-size()`.
    fn from_calc_size(f: CalcSizeFunction) -> Self;
}

macro_rules! impl_as_calc_size {
    ($ty:ident, $generic:ident) => {
        impl AsCalcSize for $ty {
            fn as_calc_size(&self, keywords: bool) -> Option<CalcSizeFunction> {
                let keyword = |basis| keywords.then(|| CalcSizeFunction::keyword(basis));
                match self {
                    $generic::LengthPercentage(lp) => Some(CalcSizeFunction::any(lp)),
                    $generic::CalcSize(f) => Some((**f).clone()),
                    $generic::MinContent => keyword(CalcSizeBasis::MinContent),
                    $generic::MaxContent => keyword(CalcSizeBasis::MaxContent),
                    $generic::FitContent => keyword(CalcSizeBasis::FitContent),
                    $generic::Stretch | $generic::WebkitFillAvailable => {
                        keyword(CalcSizeBasis::Stretch)
                    },
                    _ => self.as_calc_size_extra(keywords),
                }
            }

            fn is_calc_size(&self) -> bool {
                matches!(self, $generic::CalcSize(..))
            }

            fn from_calc_size(f: CalcSizeFunction) -> Self {
                $generic::CalcSize(Box::new(f))
            }
        }

        impl ToAnimatedValue for $ty {
            type AnimatedValue = AnimatedSize<$ty>;

            fn to_animated_value(self, context: &AnimatedContext) -> Self::AnimatedValue {
                let allow_keywords = context.style.get_inherited_box().interpolate_size ==
                    InterpolateSize::AllowKeywords;
                // (lengths animate unzoomed, as the derived conversion of the other variants does)
                let value = match self {
                    $generic::LengthPercentage(lp) => {
                        $generic::LengthPercentage(lp.to_animated_value(context))
                    },
                    $generic::FitContentFunction(lp) => {
                        $generic::FitContentFunction(lp.to_animated_value(context))
                    },
                    $generic::AnchorContainingCalcFunction(lp) => {
                        $generic::AnchorContainingCalcFunction(lp.to_animated_value(context))
                    },
                    $generic::CalcSize(f) => $generic::CalcSize(Box::new(f.unzoomed(context))),
                    value => value,
                };
                AnimatedSize {
                    value,
                    allow_keywords,
                }
            }

            fn from_animated_value(animated: Self::AnimatedValue) -> Self {
                // (an extrapolated length is clamped as the property's values are)
                match animated.value {
                    $generic::LengthPercentage(lp) => $generic::LengthPercentage(
                        NonNegativeLengthPercentage::from_animated_value(lp),
                    ),
                    $generic::FitContentFunction(lp) => $generic::FitContentFunction(
                        NonNegativeLengthPercentage::from_animated_value(lp),
                    ),
                    value => value,
                }
            }
        }
    };
}

impl Size {
    fn as_calc_size_extra(&self, keywords: bool) -> Option<CalcSizeFunction> {
        match self {
            GenericSize::Auto if keywords => Some(CalcSizeFunction::keyword(CalcSizeBasis::Auto)),
            _ => None,
        }
    }
}

impl MaxSize {
    fn as_calc_size_extra(&self, _keywords: bool) -> Option<CalcSizeFunction> {
        None
    }
}

impl_as_calc_size!(Size, GenericSize);
impl_as_calc_size!(MaxSize, GenericMaxSize);

/// The animated value of a sizing property: the computed value and whether the element's
/// `interpolate-size` is `allow-keywords` (§7.3).
#[derive(Clone, Debug, MallocSizeOf, PartialEq)]
pub struct AnimatedSize<S> {
    /// The value.
    pub value: S,
    /// `interpolate-size: allow-keywords` on the element.
    pub allow_keywords: bool,
}

impl<S> Animate for AnimatedSize<S>
where
    S: Animate + AsCalcSize,
{
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        // §7.3: the value on the element when the animation starts. For a transition that is
        // the after-change style, the `to` side.
        let allow_keywords = other.allow_keywords;
        if let Ok(value) = self.value.animate(&other.value, procedure) {
            return Ok(Self {
                value,
                allow_keywords,
            });
        }
        // A keyword combines with a `calc-size()` always (`numeric-only`), and with a
        // `<length-percentage>` under `allow-keywords`.
        let keywords =
            allow_keywords || self.value.is_calc_size() || other.value.is_calc_size();
        let a = self.value.as_calc_size(keywords).ok_or(())?;
        let b = other.value.as_calc_size(keywords).ok_or(())?;
        Ok(Self {
            value: S::from_calc_size(a.animate_with(&b, procedure)?),
            allow_keywords,
        })
    }
}

impl<S: ComputeSquaredDistance> ComputeSquaredDistance for AnimatedSize<S> {
    fn compute_squared_distance(&self, other: &Self) -> Result<SquaredDistance, ()> {
        self.value.compute_squared_distance(&other.value)
    }
}

impl<S: ToAnimatedZero> ToAnimatedZero for AnimatedSize<S> {
    fn to_animated_zero(&self) -> Result<Self, ()> {
        Ok(Self {
            value: self.value.to_animated_zero()?,
            allow_keywords: self.allow_keywords,
        })
    }
}
