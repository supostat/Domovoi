use std::str::FromStr;

use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};

use crate::theme_tokens::{PaletteValue, ThemeTokens};
use crate::theme_value::{Color, Shadow};

pub(crate) fn source(tokens: &ThemeTokens) -> String {
    let palette_fields: Vec<_> = tokens.palette.iter().map(|token| field(&token.name)).collect();
    let palette_types = tokens.palette.iter().map(|token| match token.dark {
        PaletteValue::Color(_) => quote!(Rgba),
        PaletteValue::Shadow(_) => quote!(Shadow),
    });
    let dark_values = tokens.palette.iter().map(|token| palette_value(token.dark));
    let light_values = tokens.palette.iter().map(|token| palette_value(token.light));
    let radius_fields: Vec<_> = tokens.radii.iter().map(|(name, _)| field(name)).collect();
    let radius_values = tokens.radii.iter().map(|(_, length)| Literal::f32_unsuffixed(*length));
    let font_stack_fields: Vec<_> = tokens.font_stacks.iter().map(|(name, _)| field(name)).collect();
    let font_stack_values = tokens.font_stacks.iter().map(|(_, families)| quote!(&[#(#families),*]));
    let module = quote! {
        use crate::rgba::Rgba;
        use crate::shadow::Shadow;

        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct Palette {
            #(pub #palette_fields: #palette_types,)*
        }

        impl Palette {
            pub const DARK: Self = Self {
                #(#palette_fields: #dark_values,)*
            };
            pub const LIGHT: Self = Self {
                #(#palette_fields: #light_values,)*
            };
        }

        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct Radii {
            #(pub #radius_fields: f32,)*
        }

        pub const RADII: Radii = Radii {
            #(#radius_fields: #radius_values,)*
        };

        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct FontStacks {
            #(pub #font_stack_fields: &'static [&'static str],)*
        }

        pub const FONT_STACKS: FontStacks = FontStacks {
            #(#font_stack_fields: #font_stack_values,)*
        };
    };
    let syntax = syn::parse2::<syn::File>(module).expect("generated theme module is valid Rust");
    prettyplease::unparse(&syntax)
}

fn field(token_name: &str) -> Ident {
    format_ident!("{}", token_name.replace('-', "_"))
}

fn palette_value(value: PaletteValue) -> TokenStream {
    match value {
        PaletteValue::Color(color) => rgba(color),
        PaletteValue::Shadow(Shadow {
            offset_x,
            offset_y,
            blur,
            color,
        }) => {
            let [offset_x, offset_y, blur] = [offset_x, offset_y, blur].map(Literal::f32_unsuffixed);
            let color = rgba(color);
            quote!(Shadow { offset_x: #offset_x, offset_y: #offset_y, blur: #blur, color: #color })
        }
    }
}

fn rgba(color: Color) -> TokenStream {
    let [red, green, blue] = [color.red, color.green, color.blue].map(hex_byte);
    match color.alpha {
        None => quote!(Rgba::opaque(#red, #green, #blue)),
        Some(alpha) => {
            let alpha = Literal::f32_unsuffixed(alpha);
            quote!(Rgba::translucent(#red, #green, #blue, #alpha))
        }
    }
}

fn hex_byte(value: u8) -> Literal {
    Literal::from_str(&format!("0x{value:02X}")).expect("a hex byte is a valid integer literal")
}
