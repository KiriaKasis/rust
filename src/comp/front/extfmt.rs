/*
 * The compiler code necessary to support the #fmt extension.  Eventually this
 * should all get sucked into either the standard library ExtFmt module or the
 * compiler syntax extension plugin interface.
 */

import util.common;

import std._str;
import std._vec;
import std.option;
import std.option.none;
import std.option.some;

import std.ExtFmt.CT.signedness;
import std.ExtFmt.CT.signed;
import std.ExtFmt.CT.unsigned;
import std.ExtFmt.CT.caseness;
import std.ExtFmt.CT.case_upper;
import std.ExtFmt.CT.case_lower;
import std.ExtFmt.CT.ty;
import std.ExtFmt.CT.ty_bool;
import std.ExtFmt.CT.ty_str;
import std.ExtFmt.CT.ty_char;
import std.ExtFmt.CT.ty_int;
import std.ExtFmt.CT.ty_bits;
import std.ExtFmt.CT.ty_hex;
import std.ExtFmt.CT.flag;
import std.ExtFmt.CT.flag_left_justify;
import std.ExtFmt.CT.flag_left_zero_pad;
import std.ExtFmt.CT.flag_left_space_pad;
import std.ExtFmt.CT.flag_plus_if_positive;
import std.ExtFmt.CT.flag_alternate;
import std.ExtFmt.CT.count;
import std.ExtFmt.CT.count_is;
import std.ExtFmt.CT.count_is_param;
import std.ExtFmt.CT.count_is_next_param;
import std.ExtFmt.CT.count_implied;
import std.ExtFmt.CT.conv;
import std.ExtFmt.CT.piece;
import std.ExtFmt.CT.piece_string;
import std.ExtFmt.CT.piece_conv;
import std.ExtFmt.CT.parse_fmt_string;

export expand_syntax_ext;

// FIXME: Need to thread parser through here to handle errors correctly
fn expand_syntax_ext(vec[@ast.expr] args,
                     option.t[@ast.expr] body) -> @ast.expr {

    if (_vec.len[@ast.expr](args) == 0u) {
        log "malformed #fmt call";
        fail;
    }

    auto fmt = expr_to_str(args.(0));

    // log "Format string:";
    // log fmt;

    auto pieces = parse_fmt_string(fmt);
    auto args_len = _vec.len[@ast.expr](args);
    auto fmt_args = _vec.slice[@ast.expr](args, 1u, args_len - 1u);
    ret pieces_to_expr(pieces, args);
}

fn expr_to_str(@ast.expr expr) -> str {
    alt (expr.node) {
        case (ast.expr_lit(?l, _)) {
            alt (l.node) {
                case (ast.lit_str(?s)) {
                    ret s;
                }
            }
        }
    }
    log "malformed #fmt call";
    fail;
}

fn pieces_to_expr(vec[piece] pieces, vec[@ast.expr] args) -> @ast.expr {

    fn make_new_lit(common.span sp, ast.lit_ lit) -> @ast.expr {
        auto sp_lit = @rec(node=lit, span=sp);
        auto expr = ast.expr_lit(sp_lit, ast.ann_none);
        ret @rec(node=expr, span=sp);
    }

    fn make_new_str(common.span sp, str s) -> @ast.expr {
        auto lit = ast.lit_str(s);
        ret make_new_lit(sp, lit);
    }

    fn make_new_uint(common.span sp, uint u) -> @ast.expr {
        auto lit = ast.lit_uint(u);
        ret make_new_lit(sp, lit);
    }

    fn make_add_expr(common.span sp,
                     @ast.expr lhs, @ast.expr rhs) -> @ast.expr {
        auto binexpr = ast.expr_binary(ast.add, lhs, rhs, ast.ann_none);
        ret @rec(node=binexpr, span=sp);
    }

    fn make_path_expr(common.span sp, vec[ast.ident] idents) -> @ast.expr {
        let vec[@ast.ty] types = vec();
        auto path = rec(idents=idents, types=types);
        auto sp_path = rec(node=path, span=sp);
        auto pathexpr = ast.expr_path(sp_path, none[ast.def], ast.ann_none);
        auto sp_pathexpr = @rec(node=pathexpr, span=sp);
        ret sp_pathexpr;
    }

    fn make_call(common.span sp, vec[ast.ident] fn_path,
                 vec[@ast.expr] args) -> @ast.expr {
        auto pathexpr = make_path_expr(sp, fn_path);
        auto callexpr = ast.expr_call(pathexpr, args, ast.ann_none);
        auto sp_callexpr = @rec(node=callexpr, span=sp);
        ret sp_callexpr;
    }

    fn make_rec_expr(common.span sp,
                     vec[tup(ast.ident, @ast.expr)] fields) -> @ast.expr {
        let vec[ast.field] astfields = vec();
        for (tup(ast.ident, @ast.expr) field in fields) {
            auto ident = field._0;
            auto val = field._1;
            auto astfield = rec(mut = ast.imm,
                                ident = ident,
                                expr = val);
            astfields += vec(astfield);
        }

        auto recexpr = ast.expr_rec(astfields,
                                    option.none[@ast.expr],
                                    ast.ann_none);
        auto sp_recexpr = @rec(node=recexpr, span=sp);
        ret sp_recexpr;
    }

    fn make_path_vec(str ident) -> vec[str] {
        // FIXME: #fmt can't currently be used from within std
        // because we're explicitly referencing the 'std' crate here
        ret vec("std", "ExtFmt", "RT", ident);
    }

    fn make_rt_conv_expr(common.span sp, &conv cnv) -> @ast.expr {
        fn make_ty(common.span sp, &ty t) -> @ast.expr {
            auto rt_type;
            alt (t) {
                case (ty_hex(?c)) {
                    alt (c) {
                        case (case_upper) {
                            rt_type = "ty_hex_upper";
                        }
                        case (case_lower) {
                            rt_type = "ty_hex_lower";
                        }
                    }
                }
                case (ty_bits) {
                    rt_type = "ty_bits";
                }
                case (_) {
                    rt_type = "ty_default";
                }
            }

            auto idents = make_path_vec(rt_type);
            ret make_path_expr(sp, idents);
        }

        fn make_conv_rec(common.span sp, &@ast.expr ty_expr) -> @ast.expr {
            ret make_rec_expr(sp, vec(tup("ty", ty_expr)));
        }

        auto rt_conv_ty = make_ty(sp, cnv.ty);
        ret make_conv_rec(sp, rt_conv_ty);
    }

    fn make_conv_call(common.span sp, str conv_type,
                      &conv cnv, @ast.expr arg) -> @ast.expr {
        auto fname = "conv_" + conv_type;
        auto path = make_path_vec(fname);
        auto cnv_expr = make_rt_conv_expr(sp, cnv);
        auto args = vec(cnv_expr, arg);
        ret make_call(arg.span, path, args);
    }

    fn make_new_conv(conv cnv, @ast.expr arg) -> @ast.expr {

        auto unsupported = "conversion not supported in #fmt string";

        alt (cnv.param) {
            case (option.none[int]) {
            }
            case (_) {
                log unsupported;
                fail;
            }
        }

        if (_vec.len[flag](cnv.flags) != 0u) {
            log unsupported;
            fail;
        }

        alt (cnv.width) {
            case (count_implied) {
            }
            case (_) {
                log unsupported;
                fail;
            }
        }

        alt (cnv.precision) {
            case (count_implied) {
            }
            case (_) {
                log unsupported;
                fail;
            }
        }

        alt (cnv.ty) {
            case (ty_str) {
                ret arg;
            }
            case (ty_int(?sign)) {
                alt (sign) {
                    case (signed) {
                        ret make_conv_call(arg.span, "int", cnv, arg);
                    }
                    case (unsigned) {
                        ret make_conv_call(arg.span, "uint", cnv, arg);
                    }
                }
            }
            case (ty_bool) {
                ret make_conv_call(arg.span, "bool", cnv, arg);
            }
            case (ty_char) {
                ret make_conv_call(arg.span, "char", cnv, arg);
            }
            case (ty_hex(_)) {
                ret make_conv_call(arg.span, "uint", cnv, arg);
            }
            case (ty_bits) {
                ret make_conv_call(arg.span, "uint", cnv, arg);
            }
            case (_) {
                log unsupported;
                fail;
            }
        }
    }

    fn log_conv(conv c) {
        alt (c.param) {
            case (some[int](?p)) {
                log "param: " + std._int.to_str(p, 10u);
            }
            case (_) {
                log "param: none";
            }
        }
        for (flag f in c.flags) {
            alt (f) {
                case (flag_left_justify) {
                    log "flag: left justify";
                }
                case (flag_left_zero_pad) {
                    log "flag: left zero pad";
                }
                case (flag_left_space_pad) {
                    log "flag: left space pad";
                }
                case (flag_plus_if_positive) {
                    log "flag: plus if positive";
                }
                case (flag_alternate) {
                    log "flag: alternate";
                }
            }
        }
        alt (c.width) {
            case (count_is(?i)) {
                log "width: count is " + std._int.to_str(i, 10u);
            }
            case (count_is_param(?i)) {
                log "width: count is param " + std._int.to_str(i, 10u);
            }
            case (count_is_next_param) {
                log "width: count is next param";
            }
            case (count_implied) {
                log "width: count is implied";
            }
        }
        alt (c.precision) {
            case (count_is(?i)) {
                log "prec: count is " + std._int.to_str(i, 10u);
            }
            case (count_is_param(?i)) {
                log "prec: count is param " + std._int.to_str(i, 10u);
            }
            case (count_is_next_param) {
                log "prec: count is next param";
            }
            case (count_implied) {
                log "prec: count is implied";
            }
        }
        alt (c.ty) {
            case (ty_bool) {
                log "type: bool";
            }
            case (ty_str) {
                log "type: str";
            }
            case (ty_char) {
                log "type: char";
            }
            case (ty_int(?s)) {
                alt (s) {
                    case (signed) {
                        log "type: signed";
                    }
                    case (unsigned) {
                        log "type: unsigned";
                    }
                }
            }
            case (ty_bits) {
                log "type: bits";
            }
            case (ty_hex(?cs)) {
                alt (cs) {
                    case (case_upper) {
                        log "type: uhex";
                    }
                    case (case_lower) {
                        log "type: lhex";
                    }
                }
            }
        }
    }

    auto sp = args.(0).span;
    auto n = 0u;
    auto tmp_expr = make_new_str(sp, "");

    for (piece p in pieces) {
        alt (p) {
            case (piece_string(?s)) {
                auto s_expr = make_new_str(sp, s);
                tmp_expr = make_add_expr(sp, tmp_expr, s_expr);
            }
            case (piece_conv(?conv)) {
                if (n >= _vec.len[@ast.expr](args)) {
                    log "too many conversions in #fmt string";
                    fail;
                }

                // TODO: Remove debug logging
                // log "Building conversion:";
                // log_conv(conv);

                n += 1u;
                auto arg_expr = args.(n);
                auto c_expr = make_new_conv(conv, arg_expr);
                tmp_expr = make_add_expr(sp, tmp_expr, c_expr);
            }
        }
    }

    // TODO: Remove this debug logging
    // log "dumping expanded ast:";
    // log pretty.print_expr(tmp_expr);
    ret tmp_expr;
}

//
// Local Variables:
// mode: rust
// fill-column: 78;
// indent-tabs-mode: nil
// c-basic-offset: 4
// buffer-file-coding-system: utf-8-unix
// compile-command: "make -k -C $RBUILD 2>&1 | sed -e 's/\\/x\\//x:\\//g'";
// End:
//
