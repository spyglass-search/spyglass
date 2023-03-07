// Copyright 2014-2021 The winit contributors
// Copyright 2021-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0

use crate::keyboard::{KeyCode, ModifiersState, NativeKeyCode};
use std::{
    borrow::Borrow,
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    str::FromStr,
};

/// Base `Accelerator` functions.
#[derive(Debug, Clone, PartialEq, Hash)]
pub struct Accelerator {
    id: Option<AcceleratorId>,
    pub mods: ModifiersState,
    pub key: KeyCode,
}

impl Accelerator {
    /// Creates a new accelerator to define keyboard shortcuts throughout your application.
    pub fn new(mods: impl Into<Option<ModifiersState>>, key: KeyCode) -> Self {
        Self {
            id: None,
            mods: mods.into().unwrap_or_else(ModifiersState::empty),
            key,
        }
    }

    /// Assign a custom accelerator id.
    pub fn with_id(mut self, id: AcceleratorId) -> Self {
        self.id = Some(id);
        self
    }

    /// Returns an identifier unique to the accelerator.
    pub fn id(self) -> AcceleratorId {
        if let Some(id) = self.id {
            return id;
        }

        AcceleratorId(hash_accelerator_to_u16(self))
    }

    /// Returns `true` if this [`KeyCode`] and [`ModifiersState`] matches this `Accelerator`.
    ///
    /// [`KeyCode`]: KeyCode
    /// [`ModifiersState`]: crate::keyboard::ModifiersState
    pub fn matches(
        &self,
        modifiers: impl Borrow<ModifiersState>,
        key: impl Borrow<KeyCode>,
    ) -> bool {
        // Should be a const but const bit_or doesn't work here.
        let base_mods = ModifiersState::SHIFT
            | ModifiersState::CONTROL
            | ModifiersState::ALT
            | ModifiersState::SUPER;
        let modifiers = modifiers.borrow();
        let key = key.borrow();
        self.mods == *modifiers & base_mods && self.key == *key
    }
}

// Accelerator::from_str is available to be backward
// compatible with tauri and it also open the option
// to generate accelerator from string
impl FromStr for Accelerator {
    type Err = AcceleratorParseError;
    fn from_str(accelerator_string: &str) -> Result<Self, Self::Err> {
        parse_accelerator(accelerator_string)
    }
}

/// Represents the platform-agnostic keyboard modifiers, for command handling.
///
/// **This does one thing: it allows specifying accelerators that use the Command key
/// on macOS, but use the Ctrl key on other platforms.**
#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub enum SysMods {
    None,
    Shift,
    /// Command on macOS, and Ctrl on windows/linux
    Cmd,
    /// Command + Alt on macOS, Ctrl + Alt on windows/linux
    AltCmd,
    /// Command + Shift on macOS, Ctrl + Shift on windows/linux
    CmdShift,
    /// Command + Alt + Shift on macOS, Ctrl + Alt + Shift on windows/linux
    AltCmdShift,
}

/// Represents the active modifier keys.
///
/// This is intended to be clearer than [`ModifiersState`], when describing accelerators.
///
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Hash)]
pub enum RawMods {
    None,
    Alt,
    Ctrl,
    Meta,
    Shift,
    AltCtrl,
    AltMeta,
    AltShift,
    CtrlShift,
    CtrlMeta,
    MetaShift,
    AltCtrlMeta,
    AltCtrlShift,
    AltMetaShift,
    CtrlMetaShift,
    AltCtrlMetaShift,
}

// we do this so that Accelerator::new can accept `None` as an initial argument.
impl From<SysMods> for Option<ModifiersState> {
    fn from(src: SysMods) -> Option<ModifiersState> {
        Some(src.into())
    }
}

impl From<RawMods> for Option<ModifiersState> {
    fn from(src: RawMods) -> Option<ModifiersState> {
        Some(src.into())
    }
}

impl From<RawMods> for ModifiersState {
    fn from(src: RawMods) -> ModifiersState {
        let (alt, ctrl, meta, shift) = match src {
            RawMods::None => (false, false, false, false),
            RawMods::Alt => (true, false, false, false),
            RawMods::Ctrl => (false, true, false, false),
            RawMods::Meta => (false, false, true, false),
            RawMods::Shift => (false, false, false, true),
            RawMods::AltCtrl => (true, true, false, false),
            RawMods::AltMeta => (true, false, true, false),
            RawMods::AltShift => (true, false, false, true),
            RawMods::CtrlMeta => (false, true, true, false),
            RawMods::CtrlShift => (false, true, false, true),
            RawMods::MetaShift => (false, false, true, true),
            RawMods::AltCtrlMeta => (true, true, true, false),
            RawMods::AltMetaShift => (true, false, true, true),
            RawMods::AltCtrlShift => (true, true, false, true),
            RawMods::CtrlMetaShift => (false, true, true, true),
            RawMods::AltCtrlMetaShift => (true, true, true, true),
        };
        let mut mods = ModifiersState::empty();
        mods.set(ModifiersState::ALT, alt);
        mods.set(ModifiersState::CONTROL, ctrl);
        mods.set(ModifiersState::SUPER, meta);
        mods.set(ModifiersState::SHIFT, shift);
        mods
    }
}

impl From<SysMods> for ModifiersState {
    fn from(src: SysMods) -> ModifiersState {
        let (alt, ctrl, meta, shift) = match src {
            SysMods::None => (false, false, false, false),
            SysMods::Shift => (false, false, false, true),

            #[cfg(target_os = "macos")]
            SysMods::AltCmd => (true, false, true, false),
            #[cfg(not(target_os = "macos"))]
            SysMods::AltCmd => (true, true, false, false),

            #[cfg(target_os = "macos")]
            SysMods::AltCmdShift => (true, false, true, true),
            #[cfg(not(target_os = "macos"))]
            SysMods::AltCmdShift => (true, true, false, true),

            #[cfg(target_os = "macos")]
            SysMods::Cmd => (false, false, true, false),
            #[cfg(not(target_os = "macos"))]
            SysMods::Cmd => (false, true, false, false),

            #[cfg(target_os = "macos")]
            SysMods::CmdShift => (false, false, true, true),
            #[cfg(not(target_os = "macos"))]
            SysMods::CmdShift => (false, true, false, true),
        };
        let mut mods = ModifiersState::empty();
        mods.set(ModifiersState::ALT, alt);
        mods.set(ModifiersState::CONTROL, ctrl);
        mods.set(ModifiersState::SUPER, meta);
        mods.set(ModifiersState::SHIFT, shift);
        mods
    }
}

impl From<SysMods> for RawMods {
    fn from(src: SysMods) -> RawMods {
        #[cfg(target_os = "macos")]
        match src {
            SysMods::None => RawMods::None,
            SysMods::Shift => RawMods::Shift,
            SysMods::Cmd => RawMods::Meta,
            SysMods::AltCmd => RawMods::AltMeta,
            SysMods::CmdShift => RawMods::MetaShift,
            SysMods::AltCmdShift => RawMods::AltMetaShift,
        }
        #[cfg(not(target_os = "macos"))]
        match src {
            SysMods::None => RawMods::None,
            SysMods::Shift => RawMods::Shift,
            SysMods::Cmd => RawMods::Ctrl,
            SysMods::AltCmd => RawMods::AltCtrl,
            SysMods::CmdShift => RawMods::CtrlShift,
            SysMods::AltCmdShift => RawMods::AltCtrlShift,
        }
    }
}

/// Identifier of an Accelerator.
///
/// Whenever you receive an event arising from a [GlobalShortcutEvent], [MenuEvent]
/// or [KeyboardInput] , this event contains a `AcceleratorId` which identifies its origin.
///
/// [MenuEvent]: crate::event::Event::MenuEvent
/// [KeyboardInput]: crate::event::WindowEvent::KeyboardInput
/// [GlobalShortcutEvent]: crate::event::Event::GlobalShortcutEvent
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct AcceleratorId(pub u16);

impl From<AcceleratorId> for u16 {
    fn from(s: AcceleratorId) -> u16 {
        s.0
    }
}

impl From<AcceleratorId> for u32 {
    fn from(s: AcceleratorId) -> u32 {
        s.0 as u32
    }
}

impl From<AcceleratorId> for i32 {
    fn from(s: AcceleratorId) -> i32 {
        s.0 as i32
    }
}

impl AcceleratorId {
    /// Return an empty `AcceleratorId`.
    pub const EMPTY: AcceleratorId = AcceleratorId(0);

    /// Create new `AcceleratorId` from a String.
    pub fn new(accelerator_string: &str) -> AcceleratorId {
        AcceleratorId(hash_string_to_u16(accelerator_string))
    }

    /// Whenever this menu is empty.
    pub fn is_empty(self) -> bool {
        Self::EMPTY == self
    }
}

#[derive(Debug, Clone)]
pub struct AcceleratorParseError(String);

impl std::fmt::Display for AcceleratorParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[AcceleratorParseError]: {}", self.0)
    }
}

fn parse_accelerator(accelerator_string: &str) -> Result<Accelerator, AcceleratorParseError> {
    let mut mods = ModifiersState::empty();
    let mut key = KeyCode::Unidentified(NativeKeyCode::Unidentified);

    for raw in accelerator_string.split('+') {
        let token = raw.trim().to_string();
        if token.is_empty() {
            return Err(AcceleratorParseError(
                "Unexpected empty token while parsing accelerator".into(),
            ));
        }

        if key != KeyCode::Unidentified(NativeKeyCode::Unidentified) {
            // at this point we already parsed the modifiers and found a main key but
            // the function received more then one main key or it is not in the right order
            // examples:
            // 1. "Ctrl+Shift+C+A" => only one main key should be allowd.
            // 2. "Ctrl+C+Shift" => wrong order
            return Err(AcceleratorParseError(format!(
                "Unexpected accelerator string format: \"{}\"",
                accelerator_string
            )));
        }

        match token.to_uppercase().as_str() {
            "OPTION" | "ALT" => {
                mods.set(ModifiersState::ALT, true);
            }
            "CONTROL" | "CTRL" => {
                mods.set(ModifiersState::CONTROL, true);
            }
            "COMMAND" | "CMD" | "SUPER" => {
                mods.set(ModifiersState::SUPER, true);
            }
            "SHIFT" => {
                mods.set(ModifiersState::SHIFT, true);
            }
            "COMMANDORCONTROL" | "COMMANDORCTRL" | "CMDORCTRL" | "CMDORCONTROL" => {
                #[cfg(target_os = "macos")]
                mods.set(ModifiersState::SUPER, true);
                #[cfg(not(target_os = "macos"))]
                mods.set(ModifiersState::CONTROL, true);
            }
            _ => {
                if let Ok(keycode) = KeyCode::from_str(&token) {
                    match keycode {
                        KeyCode::Unidentified(_) => {
                            return Err(AcceleratorParseError(format!(
                                "Couldn't identify \"{}\" as a valid `KeyCode`",
                                token
                            )))
                        }
                        _ => key = keycode,
                    }
                }
            }
        }
    }

    Ok(Accelerator {
        // use the accelerator string as id
        id: Some(AcceleratorId(hash_string_to_u16(accelerator_string))),
        key,
        mods,
    })
}

fn hash_string_to_u16(title: &str) -> u16 {
    let mut s = DefaultHasher::new();
    // we transform to uppercase to make sure
    // if we write Shift instead of SHIFT it return
    // the same ID
    title.to_uppercase().hash(&mut s);
    s.finish() as u16
}

fn hash_accelerator_to_u16(hotkey: Accelerator) -> u16 {
    let mut s = DefaultHasher::new();
    hotkey.hash(&mut s);
    s.finish() as u16
}

#[test]
fn test_parse_accelerator() {
    assert_eq!(
        parse_accelerator("CTRL+X").unwrap(),
        Accelerator {
            id: Some(AcceleratorId::new("CTRL+X")),
            mods: ModifiersState::CONTROL,
            key: KeyCode::KeyX,
        }
    );
    assert_eq!(
        parse_accelerator("SHIFT+C").unwrap(),
        Accelerator {
            id: Some(AcceleratorId::new("SHIFT+C")),
            mods: ModifiersState::SHIFT,
            key: KeyCode::KeyC,
        }
    );
    assert_eq!(
        parse_accelerator("CTRL+Z").unwrap(),
        Accelerator {
            id: Some(AcceleratorId::new("CTRL+Z")),
            mods: ModifiersState::CONTROL,
            key: KeyCode::KeyZ,
        }
    );
    assert_eq!(
        parse_accelerator("super+ctrl+SHIFT+alt+Up").unwrap(),
        Accelerator {
            id: Some(AcceleratorId::new("super+ctrl+SHIFT+alt+Up")),
            mods: ModifiersState::SUPER
                | ModifiersState::CONTROL
                | ModifiersState::SHIFT
                | ModifiersState::ALT,
            key: KeyCode::ArrowUp,
        }
    );
    assert_eq!(
        parse_accelerator("5").unwrap(),
        Accelerator {
            id: Some(AcceleratorId::new("5")),
            mods: ModifiersState::empty(),
            key: KeyCode::Digit5,
        }
    );
    assert_eq!(
        parse_accelerator("G").unwrap(),
        Accelerator {
            id: Some(AcceleratorId::new("G")),
            mods: ModifiersState::empty(),
            key: KeyCode::KeyG,
        }
    );
    assert_eq!(
        parse_accelerator("G").unwrap(),
        Accelerator {
            // id not with same uppercase should work
            id: Some(AcceleratorId::new("g")),
            mods: ModifiersState::empty(),
            key: KeyCode::KeyG,
        }
    );

    let acc = parse_accelerator("+G");
    assert!(acc.is_err());

    let acc = parse_accelerator("SHGSH+G");
    assert!(acc.is_err());

    assert_eq!(
        parse_accelerator("SHiFT+F12").unwrap(),
        Accelerator {
            id: Some(AcceleratorId::new("SHIFT+F12")),
            mods: ModifiersState::SHIFT,
            key: KeyCode::F12,
        }
    );
    assert_eq!(
        parse_accelerator("CmdOrCtrl+Space").unwrap(),
        Accelerator {
            id: Some(AcceleratorId::new("CmdOrCtrl+Space")),
            #[cfg(target_os = "macos")]
            mods: ModifiersState::SUPER,
            #[cfg(not(target_os = "macos"))]
            mods: ModifiersState::CONTROL,
            key: KeyCode::Space,
        }
    );

    let acc = parse_accelerator("CTRL+");
    assert!(acc.is_err());
}
