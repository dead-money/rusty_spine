use miniquad::*;
use rusty_spine::BlendMode;

/// Convert a [`rusty_spine::BlendMode`] to a pair of [`miniquad::BlendState`]s. One for alpha, one
/// for color.
///
/// Spine supports 4 different blend modes:
/// - [`rusty_spine::BlendMode::Additive`]
/// - [`rusty_spine::BlendMode::Multiply`]
/// - [`rusty_spine::BlendMode::Normal`]
/// - [`rusty_spine::BlendMode::Screen`]
///
/// And blend states are different depending on if the texture has premultiplied alpha values.
///
/// So, 8 blend states must be supported. See [`GetBlendStates::get_blend_states`] below.
pub struct BlendStates {
    pub alpha_blend: BlendState,
    pub color_blend: BlendState,
}

pub trait GetBlendStates {
    fn get_blend_states(&self, premultiplied_alpha: bool) -> BlendStates;
}

impl GetBlendStates for BlendMode {
    fn get_blend_states(&self, premultiplied_alpha: bool) -> BlendStates {
        match self {
            Self::Additive => match premultiplied_alpha {
                // Case 1: Additive Blend Mode, Normal Alpha
                false => BlendStates {
                    alpha_blend: BlendState::new(Equation::Add, BlendFactor::One, BlendFactor::One),
                    color_blend: BlendState::new(
                        Equation::Add,
                        BlendFactor::Value(BlendValue::SourceAlpha),
                        BlendFactor::One,
                    ),
                },
                // Case 2: Additive Blend Mode, Premultiplied Alpha
                true => BlendStates {
                    alpha_blend: BlendState::new(Equation::Add, BlendFactor::One, BlendFactor::One),
                    color_blend: BlendState::new(Equation::Add, BlendFactor::One, BlendFactor::One),
                },
            },
            Self::Multiply => match premultiplied_alpha {
                // Case 3: Multiply Blend Mode, Normal Alpha
                false => BlendStates {
                    alpha_blend: BlendState::new(
                        Equation::Add,
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    ),
                    color_blend: BlendState::new(
                        Equation::Add,
                        BlendFactor::Value(BlendValue::DestinationColor),
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    ),
                },
                // Case 4: Multiply Blend Mode, Premultiplied Alpha
                true => BlendStates {
                    alpha_blend: BlendState::new(
                        Equation::Add,
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    ),
                    color_blend: BlendState::new(
                        Equation::Add,
                        BlendFactor::Value(BlendValue::DestinationColor),
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    ),
                },
            },
            Self::Normal => match premultiplied_alpha {
                // Case 5: Normal Blend Mode, Normal Alpha
                false => BlendStates {
                    alpha_blend: BlendState::new(
                        Equation::Add,
                        BlendFactor::One,
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    ),
                    color_blend: BlendState::new(
                        Equation::Add,
                        BlendFactor::Value(BlendValue::SourceAlpha),
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    ),
                },
                // Case 6: Normal Blend Mode, Premultiplied Alpha
                true => BlendStates {
                    alpha_blend: BlendState::new(
                        Equation::Add,
                        BlendFactor::One,
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    ),
                    color_blend: BlendState::new(
                        Equation::Add,
                        BlendFactor::One,
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    ),
                },
            },
            Self::Screen => match premultiplied_alpha {
                // Case 7: Screen Blend Mode, Normal Alpha
                false => BlendStates {
                    alpha_blend: BlendState::new(
                        Equation::Add,
                        BlendFactor::OneMinusValue(BlendValue::SourceColor),
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    ),
                    color_blend: BlendState::new(
                        Equation::Add,
                        BlendFactor::One,
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    ),
                },
                // Case 8: Screen Blend Mode, Premultiplied Alpha
                true => BlendStates {
                    alpha_blend: BlendState::new(
                        Equation::Add,
                        BlendFactor::OneMinusValue(BlendValue::SourceColor),
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    ),
                    color_blend: BlendState::new(
                        Equation::Add,
                        BlendFactor::One,
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    ),
                },
            },
        }
    }
}
