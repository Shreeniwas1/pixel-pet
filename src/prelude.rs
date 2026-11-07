//! One place for the imports every module needs, so each file starts with `use crate::prelude::*;`.
#![allow(unused_imports)]

pub(crate) use std::mem::{size_of, zeroed};
pub(crate) use std::ptr::{null, null_mut};
pub(crate) use std::sync::atomic::{AtomicI32, AtomicIsize, Ordering::Relaxed};
pub(crate) use std::sync::Mutex;
pub(crate) use std::time::{Duration, Instant};

pub(crate) use windows_sys::Win32::Foundation::*;
pub(crate) use windows_sys::Win32::Graphics::Gdi::*;
pub(crate) use windows_sys::Win32::NetworkManagement::IpHelper::*;
pub(crate) use windows_sys::Win32::System::Diagnostics::Debug::MessageBeep;
pub(crate) use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
pub(crate) use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
pub(crate) use windows_sys::Win32::System::SystemInformation::{GetLocalTime, GetTickCount, GlobalMemoryStatusEx, MEMORYSTATUSEX};
pub(crate) use windows_sys::Win32::System::Threading::GetSystemTimes;
pub(crate) use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
pub(crate) use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub(crate) use crate::care::{self, Coach, Watch};
pub(crate) use crate::mood::{self, Act as Pose, Ev, Mood};
pub(crate) use crate::stats::Stats;
pub(crate) use crate::todo::{self, Todo};
pub(crate) use crate::{intro, rules};

pub(crate) use crate::art::*;
pub(crate) use crate::gfx::*;
pub(crate) use crate::hub::*;
pub(crate) use crate::paint::*;
pub(crate) use crate::petwnd::*;
pub(crate) use crate::place::*;
pub(crate) use crate::state::*;
pub(crate) use crate::statswin::*;
pub(crate) use crate::store::*;
pub(crate) use crate::stroll::*;
pub(crate) use crate::sys::*;
pub(crate) use crate::talk::*;
pub(crate) use crate::tick::*;
pub(crate) use crate::todowin::*;
