use std::collections::VecDeque;

use crate::bitstream_util::SyntaxField;
use crate::bitstream_util::SyntaxNode;
use crate::bitstream_util::SyntaxElement;
use crate::bitstream_util::BitstreamReader;
use crate::bitstream_util::BitstreamWriter;
use crate::bitstream_util::FieldType;
use crate::bitstream_util::BitstreamProcessor;
use crate::bitstream_util::syntax_elements_from_string;

struct H264State {
    chroma_format_idc: i32,
    separate_color_plane_flag: bool,
    frame_mbs_only_flag: bool,
    pic_order_cnt_type: i32,
    bottom_field_pic_order_in_frame_present_flag: bool,
    delta_pic_order_always_zero_flag: bool,
    redundant_pic_cnt_present_flag: bool,
    weighted_pred_flag: bool,
    weighted_bipred_idc: i32,
    entropy_coding_mode_flag: bool,
    deblocking_filter_control_present_flag: bool,
    num_slice_groups_minus1: i32,
    slice_group_map_type: i32,
    log2_max_frame_num_minus4: i32,
    log2_max_pic_order_cnt_lsb_minus4: i32,
    num_ref_idx_l0_active_minus1: i32,
    num_ref_idx_l1_active_minus1: i32,
    pic_size_in_map_units_minus1: i32,
    slice_group_change_rate_minus1: i32,
}

impl H264State {
    fn new() -> H264State {
        H264State { chroma_format_idc: 1,
                    separate_color_plane_flag: false,
                    frame_mbs_only_flag: false,
                    pic_order_cnt_type: 0,
                    bottom_field_pic_order_in_frame_present_flag: false,
                    delta_pic_order_always_zero_flag: false,
                    redundant_pic_cnt_present_flag: false,
                    weighted_pred_flag: false,
                    weighted_bipred_idc: 0,
                    entropy_coding_mode_flag: false,
                    deblocking_filter_control_present_flag: false,
                    num_slice_groups_minus1: 0,
                    slice_group_map_type: 0,
                    log2_max_frame_num_minus4: 0,
                    log2_max_pic_order_cnt_lsb_minus4: 0,
                    num_ref_idx_l0_active_minus1: 0,
                    num_ref_idx_l1_active_minus1: 0,
                    pic_size_in_map_units_minus1: 0,
                    slice_group_change_rate_minus1: 0,
        }
    }
}

#[derive(PartialEq)]
enum SliceType {
    P,
    B,
    I,
    SP,
    SI,
}

fn int_to_slice_type(x: i32) -> SliceType {
    match x % 5 {
        0 => SliceType::P,
        1 => SliceType::B,
        2 => SliceType::I,
        3 => SliceType::SP,
        4 => SliceType::SI,
        _ => panic!("not possible"),
    }
}

fn tokenize_h264_bitstream(bitstream: &Vec<u8>) -> Vec<BitstreamReader> {
    let mut ret: Vec<BitstreamReader> = vec![];
    let mut start_idx = 0;
    let mut curr_idx = 0;
    while curr_idx < bitstream.len() {
        if curr_idx < bitstream.len() - 4 &&
            bitstream[curr_idx] == 0x00 &&
            bitstream[curr_idx+1] == 0x00 &&
            bitstream[curr_idx+2] == 0x00 &&
            bitstream[curr_idx+3] == 0x01 {
            if curr_idx != start_idx {
                ret.push(BitstreamReader::new(&bitstream[start_idx..curr_idx]));
            }
            curr_idx += 4;
            start_idx = curr_idx;
        } else if curr_idx < bitstream.len() - 3 &&
            bitstream[curr_idx] == 0x00 &&
            bitstream[curr_idx+1] == 0x00 &&
            bitstream[curr_idx+2] == 0x01 {
            if curr_idx != start_idx {
                ret.push(BitstreamReader::new(&bitstream[start_idx..curr_idx]));
            }
            curr_idx += 3;
            start_idx = curr_idx;
        } else {
            curr_idx += 1;
        }
    }
    if curr_idx != start_idx {
        ret.push(BitstreamReader::new(&bitstream[start_idx..curr_idx]));
    }

    ret
}

fn process_scaling_list<A>(node: &mut SyntaxNode, bitstream: &mut A, scaling_list_size: usize) -> ()
    where A: BitstreamProcessor {
    let mut last_scale = 8;
    let mut next_scale = 8;
    for i in 0..scaling_list_size {
        if next_scale != 0 {
            let delta_scale = bitstream.field(node, "delta_scale", FieldType::SignedExpGolomb, 0);
            next_scale = (last_scale + delta_scale + 256) % 256;
        }
        let curr_scale = if next_scale == 0 { last_scale } else { next_scale };
        last_scale = curr_scale;
    }
}

fn process_sps<A>(node: &mut SyntaxNode, bitstream: &mut A, state: &mut H264State) -> ()
    where A: BitstreamProcessor {
    let profile_idc = bitstream.field(node, "profile_idc", FieldType::UnsignedInt, 8);
    bitstream.field(node, "constraint_set0_flag", FieldType::Boolean, 1);
    bitstream.field(node, "constraint_set1_flag", FieldType::Boolean, 1);
    bitstream.field(node, "constraint_set2_flag", FieldType::Boolean, 1);
    bitstream.field(node, "constraint_set3_flag", FieldType::Boolean, 1);
    bitstream.field(node, "constraint_set4_flag", FieldType::Boolean, 1);
    bitstream.field(node, "constraint_set5_flag", FieldType::Boolean, 1);
    bitstream.field(node, "reserved_zero_2bits", FieldType::UnsignedInt, 2);
    bitstream.field(node, "level_idc", FieldType::UnsignedInt, 8);
    bitstream.field(node, "seq_paramter_set_id", FieldType::UnsignedExpGolomb, 0);
    if profile_idc == 100 ||
       profile_idc == 110 ||
       profile_idc == 122 ||
       profile_idc == 244 ||
       profile_idc == 44 ||
       profile_idc == 83 ||
       profile_idc == 86 ||
       profile_idc == 118 ||
       profile_idc == 128 ||
       profile_idc == 138 ||
       profile_idc == 139 ||
       profile_idc == 134 ||
       profile_idc == 135 {
           let chroma_format_idc = bitstream.field(node, "chroma_format_idc", FieldType::UnsignedExpGolomb, 0);
           state.chroma_format_idc = chroma_format_idc;
           if chroma_format_idc == 3 {
               state.separate_color_plane_flag = bitstream.field(node, "separate_color_plane_flag", FieldType::Boolean, 1) != 0;
           }
           bitstream.field(node, "bit_depth_luma_minus8", FieldType::UnsignedExpGolomb, 0);
           bitstream.field(node, "bit_depth_chroma_minus8", FieldType::UnsignedExpGolomb, 0);
           bitstream.field(node, "qpprime_y_zero_transform_bypass_flag", FieldType::Boolean, 1);
           let seq_scaling_matrix_present_flag = bitstream.field(node, "seq_scaling_matrix_present_flag", FieldType::Boolean, 1);
           if seq_scaling_matrix_present_flag != 0 {
               for i in 0..(if chroma_format_idc != 3 { 8 } else { 12 }) {
                   let scale_list_present = bitstream.field(node, &format!("seq_scaling_list_present_flag[{}]", i), FieldType::Boolean, 1) != 0;
                   if scale_list_present {
                       if i < 6 {
                           bitstream.subnode(node, "scaling_list4x4", |x, y| process_scaling_list(x, y, 16));
                       } else {
                           bitstream.subnode(node, "scaling_list8x8", |x, y| process_scaling_list(x, y, 64));
                       }
                   }
               }
           }
    }
    state.log2_max_frame_num_minus4 = bitstream.field(node, "log2_max_frame_num_minus4", FieldType::UnsignedExpGolomb, 0);
    let pic_order_cnt_type = bitstream.field(node, "pic_order_cnt_type", FieldType::UnsignedExpGolomb, 0);
    state.pic_order_cnt_type = pic_order_cnt_type;
    if pic_order_cnt_type == 0 {
        state.log2_max_pic_order_cnt_lsb_minus4 = bitstream.field(node, "log2_max_pic_order_cnt_lsb_minus4", FieldType::UnsignedExpGolomb, 0);
    } else if pic_order_cnt_type == 1 {
        state.delta_pic_order_always_zero_flag = bitstream.field(node, "delta_pic_order_always_zero_flag", FieldType::Boolean, 1) != 0;
        bitstream.field(node, "offset_for_non_ref_pic", FieldType::SignedExpGolomb, 0);
        bitstream.field(node, "offset_for_top_to_bottom_field", FieldType::SignedExpGolomb, 0);
        let num_ref_frames_in_pic_order_cnt_cycle = bitstream.field(node, "num_ref_frames_in_pic_order_cnt_cycle", FieldType::UnsignedExpGolomb, 0);
        for i in 0..num_ref_frames_in_pic_order_cnt_cycle {
            bitstream.field(node, &format!("offset_for_ref_frame[{}]", i), FieldType::SignedExpGolomb, 0);
        }
    }
    bitstream.field(node, "max_num_ref_frames", FieldType::UnsignedExpGolomb, 0);
    bitstream.field(node, "gaps_in_frame_num_value_allowed_flag", FieldType::Boolean, 1);
    bitstream.field(node, "pic_width_in_mbs_minus1", FieldType::UnsignedExpGolomb, 0);
    bitstream.field(node, "pic_height_in_mbs_minus1", FieldType::UnsignedExpGolomb, 0);
    let frame_mbs_only_flag = bitstream.field(node, "frame_mbs_only_flag", FieldType::Boolean, 1);
    state.frame_mbs_only_flag = frame_mbs_only_flag != 0;
    if frame_mbs_only_flag == 0 {
        bitstream.field(node, "mb_adaptive_frame_field_flag", FieldType::Boolean, 1);
    }
    bitstream.field(node, "direct_8x8_inference_flag", FieldType::Boolean, 1);
    let frame_cropping_flag = bitstream.field(node, "frame_cropping_flag", FieldType::Boolean, 1);
    if frame_cropping_flag != 0 {
        bitstream.field(node, "frame_crop_left_offset", FieldType::UnsignedExpGolomb, 0);
        bitstream.field(node, "frame_crop_right_offset", FieldType::UnsignedExpGolomb, 0);
        bitstream.field(node, "frame_crop_top_offset", FieldType::UnsignedExpGolomb, 0);
        bitstream.field(node, "frame_crop_bottom_offset", FieldType::UnsignedExpGolomb, 0);
    }
    let vui_params = bitstream.field(node, "vui_parameters_present_flag", FieldType::Boolean, 1);
    bitstream.payload(node, if vui_params != 0 { "unparsed_vui_params" } else { "trailing_bits" });
}

fn process_pps<A>(node: &mut SyntaxNode, bitstream: &mut A, state: &mut H264State) -> ()
    where A: BitstreamProcessor {
    bitstream.field(node, "pic_parameter_set_id", FieldType::UnsignedExpGolomb, 0);
    bitstream.field(node, "seq_parameter_set_id", FieldType::UnsignedExpGolomb, 0);
    state.entropy_coding_mode_flag = bitstream.field(node, "entropy_coding_mode_flag", FieldType::Boolean, 1) != 0;
    state.bottom_field_pic_order_in_frame_present_flag = bitstream.field(node, "bottom_field_pic_order_in_frame_present_flag", FieldType::Boolean, 1) != 0;
    let num_slice_groups_minus1 = bitstream.field(node, "num_slice_groups_minus1", FieldType::UnsignedExpGolomb, 0);
    state.num_slice_groups_minus1 = num_slice_groups_minus1;
    if num_slice_groups_minus1 > 0 {
        let slice_group_map_type = bitstream.field(node, "slice_group_map_type", FieldType::UnsignedExpGolomb, 0);
        state.slice_group_map_type = slice_group_map_type;
        if slice_group_map_type == 0 {
            for i in 0..(num_slice_groups_minus1+1) {
                bitstream.field(node, &format!("run_length_minus1[{}]", i), FieldType::UnsignedExpGolomb, 0);
            }
        } else if slice_group_map_type == 2 {
            for i in 0..num_slice_groups_minus1 {
                bitstream.field(node, &format!("top_left[{}]", i), FieldType::UnsignedExpGolomb, 0);
                bitstream.field(node, &format!("bottom_right[{}]", i), FieldType::UnsignedExpGolomb, 0);
            }
        } else if slice_group_map_type >= 3 && slice_group_map_type <= 5 {
            bitstream.field(node, "slice_group_change_direction_flag", FieldType::Boolean, 1);
            state.slice_group_change_rate_minus1 = bitstream.field(node, "slice_group_change_rate_minus1", FieldType::UnsignedExpGolomb, 0);
        } else if slice_group_map_type == 6 {
            let pic_size_in_map_units_minus1 = bitstream.field(node, "pic_size_in_map_units_minus1", FieldType::UnsignedExpGolomb, 0);
            state.pic_size_in_map_units_minus1 = pic_size_in_map_units_minus1;
            for i in 0..(pic_size_in_map_units_minus1+1) {
                bitstream.field(node, &format!("slice_group_id[{}]", i), FieldType::UnsignedInt, f64::from(num_slice_groups_minus1+1).log2().ceil() as u8);
            }
        }
    }
    bitstream.field(node, "num_ref_idx_l0_default_active_minus1", FieldType::UnsignedExpGolomb, 0);
    bitstream.field(node, "num_ref_idx_l1_default_active_minus1", FieldType::UnsignedExpGolomb, 0);
    state.weighted_pred_flag = bitstream.field(node, "weighted_pred_flag", FieldType::Boolean, 1) != 0;
    state.weighted_bipred_idc = bitstream.field(node, "weighted_bipred_idc", FieldType::UnsignedInt, 2);
    bitstream.field(node, "pic_init_qp_minus26", FieldType::SignedExpGolomb, 0);
    bitstream.field(node, "pic_init_qs_minus26", FieldType::SignedExpGolomb, 0);
    bitstream.field(node, "chroma_qp_index_offset", FieldType::SignedExpGolomb, 0);
    state.deblocking_filter_control_present_flag = bitstream.field(node, "deblocking_filter_control_present_flag", FieldType::Boolean, 1) != 0;
    bitstream.field(node, "constrained_intra_pred_flag", FieldType::Boolean, 1);
    state.redundant_pic_cnt_present_flag = bitstream.field(node, "redundant_pic_cnt_present_flag", FieldType::Boolean, 1) != 0;
    if bitstream.more_data(node) {
        let transform_8x8_mode_flag = bitstream.field(node, "transform_8x8_mode_flag", FieldType::Boolean, 1);
        let pic_scaling_matrix_present_flag = bitstream.field(node, "pic_scaling_matrix_present_flag", FieldType::Boolean, 1);
        if pic_scaling_matrix_present_flag != 0 {
            for i in 0..(6 + transform_8x8_mode_flag * (if state.chroma_format_idc != 3 { 2 } else { 6 })) {
                let scale_list_present = bitstream.field(node, &format!("pic_scaling_list_present_flag[{}]", i), FieldType::Boolean, 1);
                if scale_list_present != 0 {
                    if i < 6 {
                        bitstream.subnode(node, "scaling_list4x4", |x, y| process_scaling_list(x, y, 16));
                    } else {
                        bitstream.subnode(node, "scaling_list8x8", |x, y| process_scaling_list(x, y, 64));
                    }
                }
            }
        }
        bitstream.field(node, "second_chroma_qp_index_offset", FieldType::SignedExpGolomb, 0);
    }
    bitstream.payload(node, "trailing_bits");
}

fn process_filler<A>(node: &mut SyntaxNode, bitstream: &mut A) -> ()
    where A: BitstreamProcessor {
    bitstream.payload(node, "filler_data");
}

fn process_ref_pic_list_modification<A>(node: &mut SyntaxNode, bitstream: &mut A, slice_type: &SliceType) -> ()
    where A: BitstreamProcessor {
    if *slice_type != SliceType::I && *slice_type != SliceType::SI {
        let ref_pic_list_modification_flag_l0 = bitstream.field(node, "ref_pic_list_modification_flag_l0", FieldType::Boolean, 1) != 0;
        if ref_pic_list_modification_flag_l0 {
            loop {
                let modification_of_pic_nums_idc = bitstream.field(node, "modification_of_pic_nums_idc", FieldType::UnsignedExpGolomb, 0);
                match modification_of_pic_nums_idc {
                    0 | 1 => bitstream.field(node, "abs_diff_pic_num_minus1", FieldType::UnsignedExpGolomb, 0),
                    2 => bitstream.field(node, "long_term_pic_num", FieldType::UnsignedExpGolomb, 0),
                    4 | 5 => bitstream.field(node, "abs_diff_view_idx_minus1", FieldType::UnsignedExpGolomb, 0),
                    _ => break,
                };
            }
        }
    }
    if *slice_type == SliceType::B {
        let ref_pic_list_modification_flag_l1 = bitstream.field(node, "ref_pic_list_modification_flag_l1", FieldType::Boolean, 1) != 0;
        if ref_pic_list_modification_flag_l1 {
            loop {
                let modification_of_pic_nums_idc = bitstream.field(node, "modification_of_pic_nums_idc", FieldType::UnsignedExpGolomb, 0);
                match modification_of_pic_nums_idc {
                    0 | 1 => bitstream.field(node, "abs_diff_pic_num_minus1", FieldType::UnsignedExpGolomb, 0),
                    2 => bitstream.field(node, "long_term_pic_num", FieldType::UnsignedExpGolomb, 0),
                    4 | 5 => bitstream.field(node, "abs_diff_view_idx_minus1", FieldType::UnsignedExpGolomb, 0),
                    _ => break,
                };
            }
        }
    }
}

fn process_pred_weight_table<A>(node: &mut SyntaxNode, bitstream: &mut A, state: &mut H264State, slice_type: &SliceType) -> ()
    where A: BitstreamProcessor {
    bitstream.field(node, "luma_log2_weight_denom", FieldType::UnsignedExpGolomb, 0);
    let chroma_array_type = if state.separate_color_plane_flag { 0 } else { state.chroma_format_idc };
    if chroma_array_type != 0 {
        bitstream.field(node, "chroma_log2_weight_denom", FieldType::UnsignedExpGolomb, 0);
    }
    for i in 0..(state.num_ref_idx_l0_active_minus1+1) {
        let luma_weight_l0_flag = bitstream.field(node, "luma_weight_l0_flag", FieldType::Boolean, 1) != 0;
        if luma_weight_l0_flag {
            bitstream.field(node, &format!("luma_weight_l0[{}]", i), FieldType::SignedExpGolomb, 0);
            bitstream.field(node, &format!("luma_offset_l0[{}]", i), FieldType::SignedExpGolomb, 0);
        }
        if chroma_array_type != 0 {
            let chroma_weight_l0_flag = bitstream.field(node, "chroma_weight_l0_flag", FieldType::Boolean, 1) != 0;
            if chroma_weight_l0_flag {
                for j in 0..2 {
                    bitstream.field(node, &format!("chroma_weight_l0[{}][{}]", i, j), FieldType::SignedExpGolomb, 0);
                    bitstream.field(node, &format!("chroma_offset_l0[{}][{}]", i, j), FieldType::SignedExpGolomb, 0);
                }
            }
        }
    }
    if *slice_type != SliceType::B {
        for i in 0..(state.num_ref_idx_l1_active_minus1+1) {
            let luma_weight_l1_flag = bitstream.field(node, "luma_weight_l1_flag", FieldType::Boolean, 1) != 0;
            if luma_weight_l1_flag {
                bitstream.field(node, &format!("luma_weight_l1[{}]", i), FieldType::SignedExpGolomb, 0);
                bitstream.field(node, &format!("luma_offset_l1[{}]", i), FieldType::SignedExpGolomb, 0);
            }
            if chroma_array_type != 0 {
                let chroma_weight_l1_flag = bitstream.field(node, "chroma_weight_l1_flag", FieldType::Boolean, 1) != 0;
                if chroma_weight_l1_flag {
                    for j in 0..2 {
                        bitstream.field(node, &format!("chroma_weight_l1[{}][{}]", i, j), FieldType::SignedExpGolomb, 0);
                        bitstream.field(node, &format!("chroma_offset_l1[{}][{}]", i, j), FieldType::SignedExpGolomb, 0);
                    }
                }
            }
        }
    }
}

fn process_dec_ref_pic_marking<A>(node: &mut SyntaxNode, bitstream: &mut A, idr_pic_flag: bool) -> ()
    where A: BitstreamProcessor {
    if idr_pic_flag {
        bitstream.field(node, "no_output_of_prior_pics_flag", FieldType::Boolean, 1);
        bitstream.field(node, "long_term_reference_flag", FieldType::Boolean, 1);
    } else {
        let adaptive_ref_pic_marking_mode_flag = bitstream.field(node, "adaptive_ref_pic_marking_mode_flag", FieldType::Boolean, 1) != 0;
        if adaptive_ref_pic_marking_mode_flag {
            loop {
                let memory_management_control_operation = bitstream.field(node, "memory_management_control_operation", FieldType::UnsignedExpGolomb, 0);
                if memory_management_control_operation == 0 {
                    break;
                }
                if memory_management_control_operation == 1 ||
                   memory_management_control_operation == 3 {
                    bitstream.field(node, "difference_of_pic_nums_minus1", FieldType::UnsignedExpGolomb, 0);
                }
                if memory_management_control_operation == 2 {
                    bitstream.field(node, "long_term_pic_num", FieldType::UnsignedExpGolomb, 0);
                }
                if memory_management_control_operation == 3 ||
                   memory_management_control_operation == 6 {
                    bitstream.field(node, "long_term_frame_idx", FieldType::UnsignedExpGolomb, 0);
                }
                if memory_management_control_operation == 4 {
                    bitstream.field(node, "max_long_term_frame_idx_plus1", FieldType::UnsignedExpGolomb, 0);
                }
            }
        }
    }
}

fn process_slice_header<A>(node: &mut SyntaxNode, bitstream: &mut A, state: &mut H264State, nalu_type: i32, nal_ref_idc: i32) -> ()
    where A: BitstreamProcessor {
    bitstream.field(node, "first_mb_in_slice", FieldType::UnsignedExpGolomb, 0);
    let slice_type = int_to_slice_type(bitstream.field(node, "slice_type", FieldType::UnsignedExpGolomb, 0));
    bitstream.field(node, "pic_parameter_set_id", FieldType::UnsignedExpGolomb, 0);
    if state.separate_color_plane_flag {
        bitstream.field(node, "color_plane_id", FieldType::UnsignedInt, 2);
    }
    let frame_num_size = state.log2_max_frame_num_minus4 + 4;
    bitstream.field(node, "frame_num", FieldType::UnsignedInt, frame_num_size.try_into().unwrap());
    let mut field_pic_flag = false;
    if !state.frame_mbs_only_flag {
        field_pic_flag = bitstream.field(node, "field_pic_flag", FieldType::Boolean, 1) != 0;
        if field_pic_flag {
            bitstream.field(node, "bottom_field_flag", FieldType::Boolean, 1);
        }
    }
    let idr_pic_flag = nalu_type == 5;
    if idr_pic_flag {
        bitstream.field(node, "idr_pic_id", FieldType::UnsignedExpGolomb, 0);
    }
    if state.pic_order_cnt_type == 0 {
        let pic_order_cnt_lsb_size = state.log2_max_pic_order_cnt_lsb_minus4 + 4;
        bitstream.field(node, "pic_order_cnt_lsb", FieldType::UnsignedInt, pic_order_cnt_lsb_size.try_into().unwrap());
        if state.bottom_field_pic_order_in_frame_present_flag && !field_pic_flag {
            bitstream.field(node, "delta_pic_order_cnt_bottom", FieldType::SignedExpGolomb, 0);
        }
    }
    if state.pic_order_cnt_type == 1 && !state.delta_pic_order_always_zero_flag {
        bitstream.field(node, "delta_pic_order_cnt", FieldType::SignedExpGolomb, 0);
    }
    if state.redundant_pic_cnt_present_flag {
        bitstream.field(node, "redundant_pic_cnt", FieldType::UnsignedExpGolomb, 0);
    }
    if slice_type == SliceType::B {
        bitstream.field(node, "direct_spatial_mv_pred_flag", FieldType::Boolean, 1);
    }
    // P, SP, or B slice
    if slice_type == SliceType::P ||
       slice_type == SliceType::SP ||
       slice_type == SliceType::B {
        let num_ref_idx_active_override_flag = bitstream.field(node, "num_ref_idx_active_override_flag", FieldType::Boolean, 1) != 0;
        if num_ref_idx_active_override_flag {
            bitstream.field(node, "num_ref_idx_l0_active_minus1", FieldType::UnsignedExpGolomb, 0);
        }
        if slice_type == SliceType::B {
            bitstream.field(node, "num_ref_idx_l1_active_minus1", FieldType::UnsignedExpGolomb, 0);
        }
    }
    bitstream.subnode(node, if (nalu_type == 20 || nalu_type == 21) { "ref_pic_list_mvc_modification" } else { "ref_pic_list_modification" },
                      |x, y| process_ref_pic_list_modification(x, y, &slice_type));
    if (state.weighted_pred_flag && (slice_type == SliceType::P || slice_type == SliceType::SP)) ||
       (state.weighted_bipred_idc == 1 && slice_type == SliceType::B) {
        bitstream.subnode(node, "pred_weight_table", |x, y| process_pred_weight_table(x, y, state, &slice_type));
    }
    if nal_ref_idc != 0 {
        bitstream.subnode(node, "dec_ref_pic_marking", |x, y| process_dec_ref_pic_marking(x, y, idr_pic_flag));
    }
    if state.entropy_coding_mode_flag && slice_type != SliceType::I && slice_type != SliceType::SI {
        bitstream.field(node, "cabac_init_idc", FieldType::UnsignedExpGolomb, 0);
    }
    bitstream.field(node, "slice_qp_delta", FieldType::SignedExpGolomb, 0);
    if slice_type == SliceType::SP || slice_type == SliceType::SI {
        if slice_type == SliceType::SP {
            bitstream.field(node, "sp_for_switch_flag", FieldType::Boolean, 1);
        }
        bitstream.field(node, "slice_qs_delta", FieldType::SignedExpGolomb, 0);
    }
    if state.deblocking_filter_control_present_flag {
        let disable_deblocking_filter_idc = bitstream.field(node, "disable_deblocking_filter_idc", FieldType::UnsignedExpGolomb, 0);
        if disable_deblocking_filter_idc != 1 {
            bitstream.field(node, "slice_alpha_c0_offset_div2", FieldType::SignedExpGolomb, 0);
            bitstream.field(node, "slice_beta_offset_div2", FieldType::SignedExpGolomb, 0);
        }
    }
    if state.num_slice_groups_minus1 > 0 && state.slice_group_map_type >= 3 && state.slice_group_map_type <= 5 {
        let slice_group_change_cycle_size = f64::from((state.pic_size_in_map_units_minus1 + 1) / (state.slice_group_change_rate_minus1 + 1) + 1).log2().ceil() as u8;
        bitstream.field(node, "slice_group_change_cycle", FieldType::UnsignedInt, slice_group_change_cycle_size);
    }
}

fn process_slice<A>(node: &mut SyntaxNode, bitstream: &mut A, state: &mut H264State, nalu_type: i32, nalu_ref_idc: i32) -> ()
    where A: BitstreamProcessor {
    bitstream.subnode(node, "slice_header", |x, y| process_slice_header(x, y, state, nalu_type, nalu_ref_idc));
    bitstream.payload(node, "slice_payload");
}

fn process_nalu<A>(node: &mut SyntaxNode, bitstream: &mut A, state: &mut H264State) -> ()
    where A: BitstreamProcessor {
    bitstream.field(node, "forbidden_zero_bit", FieldType::Boolean, 1);
    let nalu_ref_idc = bitstream.field(node, "nal_ref_idc", FieldType::UnsignedInt, 2);
    let nalu_type = bitstream.field(node, "nal_unit_type", FieldType::UnsignedInt, 5);
    match nalu_type {
        1 | 2 | 3 | 4 | 5 => bitstream.subnode(node, "slice", |x, y| process_slice(x, y, state, nalu_type, nalu_ref_idc)),
        7 => bitstream.subnode(node, "sps", |x, y| process_sps(x, y, state)),
        8 => bitstream.subnode(node, "pps", |x, y| process_pps(x, y, state)),
        12 => bitstream.subnode(node, "filler_nalu", process_filler),
        _ => bitstream.subnode(node, "unparsed_nalu", process_filler),
    };
}

pub fn parse_h264<'a>(bitstream: &Vec<u8>) -> Vec<SyntaxElement> {
    let mut ret: Vec<SyntaxElement> = vec![];
    let mut compressed_nalus = tokenize_h264_bitstream(bitstream);
    let mut state = H264State::new();

    for mut reader in &mut compressed_nalus {
        let mut root = SyntaxNode {name: "nalu".to_string(), children: VecDeque::new()};
        process_nalu(&mut root, reader, &mut state);
        ret.push(SyntaxElement::Node(root));
    }

    ret
}

pub fn serialize_h264(human_readable: String) -> Vec<u8> {
    let mut rows: VecDeque<String> = VecDeque::from_iter(human_readable.split('\n').map(|x| x.to_string()));
    let mut nalus: VecDeque<SyntaxElement> = syntax_elements_from_string(&mut rows);
    let mut writer: BitstreamWriter = BitstreamWriter::new();
    let mut state = H264State::new();

    while nalus.len() > 0 {
        writer.write(FieldType::UnsignedInt, 8, 0x00);
        writer.write(FieldType::UnsignedInt, 8, 0x00);
        writer.write(FieldType::UnsignedInt, 8, 0x00);
        writer.write(FieldType::UnsignedInt, 8, 0x01);
        let SyntaxElement::Node(mut nalu) = nalus.pop_front().unwrap() else {
            panic!("Invalid syntax element!");
        };
        process_nalu(&mut nalu, &mut writer, &mut state);
    }

    writer.buffer
}
