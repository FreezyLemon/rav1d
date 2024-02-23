use crate::src::levels::BlockLevel;
use crate::src::levels::BL_128X128;
use crate::src::levels::BL_16X16;
use crate::src::levels::BL_32X32;
use crate::src::levels::BL_64X64;
use crate::src::levels::BL_8X8;
use std::iter;
use std::ptr;
use std::slice;

pub type EdgeFlags = u8;
pub const EDGE_I420_LEFT_HAS_BOTTOM: EdgeFlags = 32;
pub const EDGE_I422_LEFT_HAS_BOTTOM: EdgeFlags = 16;
pub const EDGE_I444_LEFT_HAS_BOTTOM: EdgeFlags = 8;
pub const EDGE_I420_TOP_HAS_RIGHT: EdgeFlags = 4;
pub const EDGE_I422_TOP_HAS_RIGHT: EdgeFlags = 2;
pub const EDGE_I444_TOP_HAS_RIGHT: EdgeFlags = 1;

pub const EDGE_LEFT_HAS_BOTTOM: EdgeFlags =
    EDGE_I444_LEFT_HAS_BOTTOM | EDGE_I422_LEFT_HAS_BOTTOM | EDGE_I420_LEFT_HAS_BOTTOM;
pub const EDGE_TOP_HAS_RIGHT: EdgeFlags =
    EDGE_I444_TOP_HAS_RIGHT | EDGE_I422_TOP_HAS_RIGHT | EDGE_I420_TOP_HAS_RIGHT;

const B: usize = 4;

#[repr(C)]
pub struct EdgeNode {
    pub o: EdgeFlags,
    pub h: [EdgeFlags; 2],
    pub v: [EdgeFlags; 2],
}

#[repr(C)]
pub struct EdgeTip {
    pub node: EdgeNode,
    pub split: [EdgeFlags; B],
}

#[repr(C)]
pub struct EdgeBranch {
    pub node: EdgeNode,
    pub tts: [EdgeFlags; 3],
    pub tbs: [EdgeFlags; 3],
    pub tls: [EdgeFlags; 3],
    pub trs: [EdgeFlags; 3],
    pub h4: [EdgeFlags; 4],
    pub v4: [EdgeFlags; 4],
    pub split: [*mut EdgeNode; B],
}

struct ModeSelMem {
    pub nwc: [*mut EdgeBranch; 3],
    pub nt: *mut EdgeTip,
}

impl EdgeTip {
    const fn new(edge_flags: EdgeFlags) -> Self {
        let o = edge_flags;
        let h = [
            edge_flags | EDGE_LEFT_HAS_BOTTOM,
            edge_flags & (EDGE_LEFT_HAS_BOTTOM | EDGE_I420_TOP_HAS_RIGHT),
        ];
        let v = [
            edge_flags | EDGE_TOP_HAS_RIGHT,
            edge_flags
                & (EDGE_TOP_HAS_RIGHT | EDGE_I420_LEFT_HAS_BOTTOM | EDGE_I422_LEFT_HAS_BOTTOM),
        ];
        let node = EdgeNode { o, h, v };

        let split = [
            EDGE_TOP_HAS_RIGHT | EDGE_LEFT_HAS_BOTTOM,
            (edge_flags & EDGE_TOP_HAS_RIGHT) | EDGE_I422_LEFT_HAS_BOTTOM,
            edge_flags | EDGE_I444_TOP_HAS_RIGHT,
            edge_flags
                & (EDGE_I420_TOP_HAS_RIGHT | EDGE_I420_LEFT_HAS_BOTTOM | EDGE_I422_LEFT_HAS_BOTTOM),
        ];

        Self { node, split }
    }
}

impl EdgeBranch {
    const fn new(edge_flags: EdgeFlags, bl: BlockLevel) -> Self {
        let o = edge_flags;
        let h = [
            edge_flags | EDGE_LEFT_HAS_BOTTOM,
            edge_flags & EDGE_LEFT_HAS_BOTTOM,
        ];
        let v = [
            edge_flags | EDGE_TOP_HAS_RIGHT,
            edge_flags & EDGE_TOP_HAS_RIGHT,
        ];
        let node = EdgeNode { o, h, v };

        let h4 = [
            edge_flags | EDGE_LEFT_HAS_BOTTOM,
            EDGE_LEFT_HAS_BOTTOM
                | (if bl == BL_16X16 {
                    edge_flags & EDGE_I420_TOP_HAS_RIGHT
                } else {
                    0 as EdgeFlags
                }),
            EDGE_LEFT_HAS_BOTTOM,
            edge_flags & EDGE_LEFT_HAS_BOTTOM,
        ];

        let v4 = [
            edge_flags | EDGE_TOP_HAS_RIGHT,
            EDGE_TOP_HAS_RIGHT
                | (if bl == BL_16X16 {
                    edge_flags & (EDGE_I420_LEFT_HAS_BOTTOM | EDGE_I422_LEFT_HAS_BOTTOM)
                } else {
                    0 as EdgeFlags
                }),
            EDGE_TOP_HAS_RIGHT,
            edge_flags & EDGE_TOP_HAS_RIGHT,
        ];

        let tls = [
            EDGE_TOP_HAS_RIGHT | EDGE_LEFT_HAS_BOTTOM,
            edge_flags & EDGE_LEFT_HAS_BOTTOM,
            edge_flags & EDGE_TOP_HAS_RIGHT,
        ];
        let trs = [
            edge_flags | EDGE_TOP_HAS_RIGHT,
            edge_flags | EDGE_LEFT_HAS_BOTTOM,
            0 as EdgeFlags,
        ];
        let tts = [
            EDGE_TOP_HAS_RIGHT | EDGE_LEFT_HAS_BOTTOM,
            edge_flags & EDGE_TOP_HAS_RIGHT,
            edge_flags & EDGE_LEFT_HAS_BOTTOM,
        ];
        let tbs = [
            edge_flags | EDGE_LEFT_HAS_BOTTOM,
            edge_flags | EDGE_TOP_HAS_RIGHT,
            0 as EdgeFlags,
        ];

        let split = [ptr::null_mut(); 4];

        Self {
            node,
            h4,
            v4,
            tls,
            trs,
            tts,
            tbs,
            split,
        }
    }
}

unsafe fn init_edges(node: *mut EdgeNode, bl: BlockLevel, edge_flags: EdgeFlags) {
    if bl == BL_8X8 {
        node.cast::<EdgeTip>().write(EdgeTip::new(edge_flags));
    } else {
        node.cast::<EdgeBranch>()
            .write(EdgeBranch::new(edge_flags, bl));
    };
}

unsafe fn init_mode_node(
    nwc: &mut EdgeBranch,
    bl: BlockLevel,
    mem: &mut ModeSelMem,
    top_has_right: bool,
    left_has_bottom: bool,
) {
    *nwc = EdgeBranch::new(
        (if top_has_right {
            EDGE_TOP_HAS_RIGHT
        } else {
            0 as EdgeFlags
        }) | (if left_has_bottom {
            EDGE_LEFT_HAS_BOTTOM
        } else {
            0 as EdgeFlags
        }),
        bl,
    );
    if bl == BL_16X16 {
        let nt = slice::from_raw_parts_mut(mem.nt, B);
        mem.nt = mem.nt.offset(B as isize);
        for (n, (split, nt)) in iter::zip(&mut nwc.split, nt).enumerate() {
            *split = &mut nt.node;
            init_edges(
                &mut nt.node,
                bl + 1,
                ((if n == 3 || (n == 1 && !top_has_right) {
                    0 as EdgeFlags
                } else {
                    EDGE_TOP_HAS_RIGHT
                }) | (if !(n == 0 || (n == 2 && left_has_bottom)) {
                    0 as EdgeFlags
                } else {
                    EDGE_LEFT_HAS_BOTTOM
                })) as EdgeFlags,
            );
        }
    } else {
        let nwc_children = slice::from_raw_parts_mut(mem.nwc[bl as usize], B);
        mem.nwc[bl as usize] = mem.nwc[bl as usize].offset(B as isize);
        for (n, (split, nwc_child)) in iter::zip(&mut nwc.split, nwc_children).enumerate() {
            *split = &mut nwc_child.node;
            init_mode_node(
                nwc_child,
                bl + 1,
                mem,
                !(n == 3 || (n == 1 && !top_has_right)),
                n == 0 || (n == 2 && left_has_bottom),
            );
        }
    };
}

const fn level_index(mut level: u8) -> isize {
    let mut level_size = 1;
    let mut index = 0;
    while level > 0 {
        index += level_size;
        level_size *= B;
        level -= 1;
    }
    index as isize
}

pub unsafe fn rav1d_init_mode_tree(root: *mut EdgeBranch, nt: &mut [EdgeTip], allow_sb128: bool) {
    let mut mem = ModeSelMem {
        nwc: [ptr::null_mut(); 3],
        nt: nt.as_mut_ptr(),
    };
    if allow_sb128 {
        mem.nwc[BL_128X128 as usize] = root.offset(level_index(1));
        mem.nwc[BL_64X64 as usize] = root.offset(level_index(2));
        mem.nwc[BL_32X32 as usize] = root.offset(level_index(3));
        init_mode_node(&mut *root, BL_128X128, &mut mem, true, false);
        assert_eq!(mem.nwc[BL_128X128 as usize], root.offset(level_index(2)));
        assert_eq!(mem.nwc[BL_64X64 as usize], root.offset(level_index(3)));
        assert_eq!(mem.nwc[BL_32X32 as usize], root.offset(level_index(4)));
    } else {
        mem.nwc[BL_128X128 as usize] = ptr::null_mut();
        mem.nwc[BL_64X64 as usize] = root.offset(level_index(1));
        mem.nwc[BL_32X32 as usize] = root.offset(level_index(2));
        init_mode_node(&mut *root, BL_64X64, &mut mem, true, false);
        assert_eq!(mem.nwc[BL_64X64 as usize], root.offset(level_index(2)));
        assert_eq!(mem.nwc[BL_32X32 as usize], root.offset(level_index(3)));
    };
    assert_eq!(mem.nt, nt.as_mut_ptr_range().end);
}

#[repr(C)]
pub struct IntraEdge128 {
    pub branch: [EdgeBranch; 85],
    pub tip: [EdgeTip; 256],
}

#[repr(C)]
pub struct IntraEdge64 {
    pub branch: [EdgeBranch; 21],
    pub tip: [EdgeTip; 64],
}

#[repr(C)]
pub struct IntraEdge {
    pub sb128: IntraEdge128,
    pub sb64: IntraEdge64,
}

impl IntraEdge {
    pub fn root(&self, bl: BlockLevel) -> &EdgeNode {
        match bl {
            BL_128X128 => &self.sb128.branch[0].node,
            BL_64X64 => &self.sb64.branch[0].node,
            _ => unreachable!(),
        }
    }
}
