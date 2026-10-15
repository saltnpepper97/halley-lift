use halley_ui::input::{InputEvent, Key, Modifiers as UiModifiers};
use smithay_client_toolkit::seat::keyboard::{Keysym, Modifiers};

/// Translate launcher editing shortcuts into the shared text editor's events.
pub fn key_event(keysym: Keysym, modifiers: Modifiers, text_len: usize) -> Option<InputEvent> {
    if modifiers.alt || modifiers.logo {
        return None;
    }
    let mut edit_modifiers = UiModifiers {
        shift: modifiers.shift,
        control: modifiers.ctrl,
        alt: false,
    };
    let key = match keysym {
        Keysym::a | Keysym::A if modifiers.ctrl => Key::Home,
        Keysym::e | Keysym::E if modifiers.ctrl => Key::End,
        Keysym::u | Keysym::U if modifiers.ctrl => {
            return Some(InputEvent::DeleteSurrounding {
                before: text_len,
                after: 0,
            });
        }
        Keysym::k | Keysym::K if modifiers.ctrl => {
            return Some(InputEvent::DeleteSurrounding {
                before: 0,
                after: text_len,
            });
        }
        Keysym::w | Keysym::W if modifiers.ctrl => Key::Backspace,
        Keysym::b | Keysym::B if modifiers.ctrl => {
            edit_modifiers.control = false;
            Key::Left
        }
        Keysym::f | Keysym::F if modifiers.ctrl => {
            edit_modifiers.control = false;
            Key::Right
        }
        Keysym::BackSpace => Key::Backspace,
        Keysym::Delete => Key::Delete,
        Keysym::Left if modifiers.shift || modifiers.ctrl => Key::Left,
        Keysym::Right if modifiers.shift || modifiers.ctrl => Key::Right,
        Keysym::Home if modifiers.ctrl => Key::Home,
        Keysym::End if modifiers.ctrl => Key::End,
        _ => return None,
    };
    Some(InputEvent::KeyDown {
        key,
        modifiers: edit_modifiers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::FontRenderer;

    fn press(renderer: &mut FontRenderer, text: &str, key: Keysym, shift: bool) -> String {
        let event = key_event(
            key,
            Modifiers {
                ctrl: true,
                shift,
                ..Modifiers::default()
            },
            text.len(),
        )
        .expect("editing shortcut");
        renderer.edit(text, event)
    }

    #[test]
    fn beginning_and_end_allow_insertion_without_replacing_the_query() {
        let mut renderer = FontRenderer::new("sans-serif").unwrap();
        let text = press(&mut renderer, "middle", Keysym::a, false);
        let text = renderer.edit(&text, InputEvent::Text("start ".into()));
        assert_eq!(text, "start middle");
        // Uppercase keysyms can be produced by Caps Lock.
        let text = press(&mut renderer, &text, Keysym::E, false);
        assert_eq!(
            renderer.edit(&text, InputEvent::Text(" end".into())),
            "start middle end"
        );
    }

    #[test]
    fn character_movement_and_line_deletion_preserve_whole_graphemes() {
        let mut renderer = FontRenderer::new("sans-serif").unwrap();
        let original = "ab👩‍💻cd";
        let mut text = press(&mut renderer, original, Keysym::a, false);
        // Deleting backward at the beginning must not delete the first character.
        text = press(&mut renderer, &text, Keysym::u, false);
        assert_eq!(text, original);
        for _ in 0..3 {
            text = press(&mut renderer, &text, Keysym::f, false);
        }
        text = press(&mut renderer, &text, Keysym::u, false);
        assert_eq!(text, "cd");
        assert_eq!(renderer.edit(&text, InputEvent::Text("é".into())), "écd");

        text = press(&mut renderer, original, Keysym::e, false);
        text = press(&mut renderer, &text, Keysym::k, false);
        assert_eq!(text, original);
        for _ in 0..3 {
            text = press(&mut renderer, &text, Keysym::b, false);
        }
        text = press(&mut renderer, &text, Keysym::k, false);
        assert_eq!(text, "ab");
        assert_eq!(renderer.edit(&text, InputEvent::Text("!".into())), "ab!");
        text = press(&mut renderer, "café 👩‍💻", Keysym::u, false);
        assert_eq!(text, "");
        assert_eq!(press(&mut renderer, &text, Keysym::u, false), "");
    }

    #[test]
    fn line_deletion_uses_the_active_cursor_even_when_text_is_selected() {
        for (key, expected) in [(Keysym::u, "ghi"), (Keysym::k, "abcDEF")] {
            let mut renderer = FontRenderer::new("sans-serif").unwrap();
            let mut text = press(&mut renderer, "abcDEFghi", Keysym::a, false);
            for _ in 0..3 {
                text = press(&mut renderer, &text, Keysym::f, false);
            }
            for _ in 0..3 {
                text = press(&mut renderer, &text, Keysym::f, true);
            }
            assert_eq!(press(&mut renderer, &text, key, false), expected);
        }
    }

    #[test]
    fn word_deletion_and_keyboard_selection_work_without_home_or_end() {
        for key in [Keysym::w, Keysym::BackSpace] {
            let mut renderer = FontRenderer::new("sans-serif").unwrap();
            let text = press(&mut renderer, "alpha café", key, false);
            assert_eq!(text, "alpha ");
            assert_eq!(press(&mut renderer, &text, key, false), "");
        }
        let mut renderer = FontRenderer::new("sans-serif").unwrap();
        let text = press(&mut renderer, "café 👩‍💻", Keysym::a, true);
        assert_eq!(press(&mut renderer, &text, Keysym::BackSpace, false), "");
        let text = press(&mut renderer, "café 👩‍💻", Keysym::a, false);
        let text = press(&mut renderer, &text, Keysym::e, true);
        assert_eq!(press(&mut renderer, &text, Keysym::Delete, false), "");
    }

    #[test]
    fn unmodified_navigation_and_other_modifier_chords_are_not_editing_shortcuts() {
        for key in [
            Keysym::Left,
            Keysym::Right,
            Keysym::Home,
            Keysym::End,
            Keysym::a,
        ] {
            assert!(key_event(key, Modifiers::default(), 10).is_none());
        }
        for modifiers in [
            Modifiers {
                ctrl: true,
                alt: true,
                ..Modifiers::default()
            },
            Modifiers {
                ctrl: true,
                logo: true,
                ..Modifiers::default()
            },
        ] {
            assert!(key_event(Keysym::a, modifiers, 10).is_none());
        }
    }
}
