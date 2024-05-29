use once_cell::sync::Lazy;
use parking_lot::RwLock;
use smash::{app::lua_bind::*, lib::lua_const::*};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    fs
};
use arcropolis_api::{arc_callback, Hash40, hash40};

macro_rules! hash40_fmt {
    ($($arg:tt)*) => {{
        arcropolis_api::hash40(format!($($arg)*).as_str())
    }}
}

struct HashCache(RwLock<HashMap<Hash40, HashMap<usize, PathBuf>>>);

impl HashCache {
    pub fn new() -> Self {
        Self(RwLock::new(HashMap::new()))
    }

    pub fn get_one_slotted_file(&self, eff_hash: Hash40, slot: usize) -> Option<PathBuf> {
        let lock = self.0.read();

        let eff_map = lock.get(&eff_hash)?;
        eff_map.get(&slot).map(|new_hash| new_hash.clone())
    }

    pub fn push_one_slotted_file(&self, real: Hash40, slot: usize, new_hash: PathBuf) {
        let mut lock = self.0.write();

        'assume_exists: {
            let Some(eff_map) = lock.get_mut(&real) else { break 'assume_exists };
            let _ = eff_map.insert(slot, new_hash);
            return;
        }

        let mut new_map = HashMap::new();
        new_map.insert(slot, new_hash);
        lock.insert(real, new_map);
    }
}
// this part taken from one slot effects : )
static mut CURRENT_EXECUTING_OBJECT: u32 = 0x50000000u32;

static SET_OFFSETS: &[usize] = &[
    0x3ac7fc, 0x3ac8f8, 0x3ac9a8, 0x3aca54, 0x3acb24, 0x3acbc0, 0x3acc6c, 0x3adf98, 0x3ae030,
    0x3adb88, 0x3adc38, 0x3adcdc, 0x3ad240, 0x3ad2f0, 0x3ad394, 0x3acda0, 0x3ace50, 0x3acef4,
    0x3ad930, 0x3ad9e0, 0x3ada84, 0x3acff0, 0x3ad0a0, 0x3ad144, 0x3ad490, 0x3ad540, 0x3ad5e4,
    0x3ad6e0, 0x3ad784, 0x3ad834, 0x6573ec, // this is on agent init
];

static UNSET_OFFSETS: &[usize] = &[
    0x3acc9c, 0x3ad870, 0x3ae06c, 0x3ade44, 0x3ad3cc, 0x3acf2c, 0x3adabc, 0x3ad178, 0x3ad61c,
];

static mut OFFSET: usize = 0usize;

#[skyline::hook(offset = OFFSET, inline)]
unsafe fn set_current_exe_obj(ctx: &skyline::hooks::InlineCtx) {
    CURRENT_EXECUTING_OBJECT = *(*ctx.registers[0].x.as_ref() as *const u32).add(2);
}

#[skyline::hook(offset = OFFSET, inline)]
unsafe fn unset_current_exe_obj(_: &skyline::hooks::InlineCtx) {
    CURRENT_EXECUTING_OBJECT = 0x50000000u32;
}

static JACK_SMASH_FILES: &[&str] = &[
    "model/bg_set/jack_p_white_color_col.nutexb",
    "model/bg_set/model.nuhlpb",
    "model/bg_set/model.numatb",
    "model/bg_set/model.numdlb",
    "model/bg_set/model.numshb",
    "model/bg_set/model.numshexb",
    "model/bg_set/model.nusktb",
    "model/bg_set/model.nusrcmdlb",
    "model/bg_set/model.xmb",
    "lut/color_grading_lut.nutexb",
];
static HASH_CACHE: Lazy<HashCache> = Lazy::new(|| {
    let jack_path = Path::new("mods:/finalsmash/jack");

    let cache = HashCache::new();


    for i in 0..8 {
        for file in JACK_SMASH_FILES {
            let path = jack_path.join(&format!("c{:02}", i)).join(file);
            if path.exists() {
                cache.push_one_slotted_file(
                    hash40(&format!("finalsmash/shared/{}", file)),
                    i,
                    path
                );
            }
        }
    }
    cache
});
#[skyline::from_offset(0x3ac560)]
unsafe fn battle_object_from_id(id: u32) -> *mut u32;
const MAX_FILE_SIZE: usize = 49_648;
#[arc_callback]
fn get_file(hash: u64, data: &mut [u8]) -> Option<usize> {
    let object_id = unsafe { CURRENT_EXECUTING_OBJECT };
    if object_id == 0x50000000u32 {
        return None;
    }

    let category = object_id >> 0x1c;
    let parent_object = match category {
        0x0 => unsafe { battle_object_from_id(object_id) as *mut smash::app::BattleObject }, //fighters
        _ => return None
    };

    let slot = unsafe { WorkModule::get_int(
        (*parent_object).module_accessor, 
        *FIGHTER_INSTANCE_WORK_ID_INT_COLOR
    ) as usize };

    if let Some(path) = HASH_CACHE.get_one_slotted_file(Hash40::from(hash), slot) {
        let res = fs::read(path).unwrap();
        data.copy_from_slice(&res);
        Some(res.len())
    } else {
        None
    }
}

#[skyline::hook(offset = 0x2359948, inline)]
unsafe fn main_menu_create(_: &skyline::hooks::InlineCtx) {
    Lazy::force(&HASH_CACHE);
}
#[skyline::main(name = "one-slot-joker-smash")]
pub fn main() {
    skyline::install_hook!(main_menu_create);
    unsafe {
        for offset in SET_OFFSETS.iter() {
            OFFSET = *offset;
            skyline::install_hook!(set_current_exe_obj);
        }
        for offset in UNSET_OFFSETS.iter() {
            OFFSET = *offset;
            skyline::install_hook!(unset_current_exe_obj);
        }
    }
    for f in JACK_SMASH_FILES {
        let hash = hash40(&format!("finalsmash/jack/shared/{}", f));
        get_file::install(hash, MAX_FILE_SIZE);
    }
}
