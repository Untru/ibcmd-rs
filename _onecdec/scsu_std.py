"""Standard SCSU (Unicode Technical Standard #6) decoder — feasibility probe."""

STATIC = [0x0000, 0x0080, 0x0100, 0x0300, 0x2000, 0x2080, 0x2100, 0x3000]
DYNAMIC = [0x0080, 0x00C0, 0x0400, 0x0600, 0x0900, 0x3040, 0x30A0, 0xFF00]
SPECIAL = {0xF9: 0x00C0, 0xFA: 0x0250, 0xFB: 0x0370, 0xFC: 0x0530,
           0xFD: 0x3040, 0xFE: 0x30A0, 0xFF: 0xFF60}


class SCSUError(ValueError):
    pass


def _offset(x):
    if 0x01 <= x <= 0x67: return x * 0x80
    if 0x68 <= x <= 0xA7: return x * 0x80 + 0xAC00
    if x in SPECIAL: return SPECIAL[x]
    raise SCSUError('reserved window offset byte 0x%02X' % x)


def decode(data: bytes) -> str:
    win = list(DYNAMIC)
    active = 0
    unicode_mode = False
    out = []              # UTF-16 code units or code points
    i, n = 0, len(data)

    def need(k):
        if i + k > n: raise SCSUError('truncated at %d' % i)

    while i < n:
        b = data[i]; i += 1
        if not unicode_mode:
            if b >= 0x80:
                out.append(win[active] + b - 0x80)
            elif b in (0x00, 0x09, 0x0A, 0x0D) or b >= 0x20:
                out.append(b)
            elif 0x01 <= b <= 0x08:                 # SQn
                need(1); c = data[i]; i += 1; k = b - 1
                out.append(STATIC[k] + c if c < 0x80 else win[k] + c - 0x80)
            elif 0x10 <= b <= 0x17:                 # SCn
                active = b - 0x10
            elif 0x18 <= b <= 0x1F:                 # SDn
                need(1); active = b - 0x18; win[active] = _offset(data[i]); i += 1
            elif b == 0x0B:                         # SDX
                need(2); h, l = data[i], data[i + 1]; i += 2
                active = h >> 5; win[active] = 0x10000 + ((((h & 0x1F) << 8) | l) << 7)
            elif b == 0x0E:                         # SQU
                need(2); out.append((data[i] << 8) | data[i + 1]); i += 2
            elif b == 0x0F:                         # SCU
                unicode_mode = True
            else:
                raise SCSUError('reserved tag 0x%02X at %d' % (b, i - 1))
        else:
            if 0xE0 <= b <= 0xE7:                   # UCn
                active = b - 0xE0; unicode_mode = False
            elif 0xE8 <= b <= 0xEF:                 # UDn
                need(1); active = b - 0xE8; win[active] = _offset(data[i]); i += 1
                unicode_mode = False
            elif b == 0xF0:                         # UQU
                need(2); out.append((data[i] << 8) | data[i + 1]); i += 2
            elif b == 0xF1:                         # UDX
                need(2); h, l = data[i], data[i + 1]; i += 2
                active = h >> 5; win[active] = 0x10000 + ((((h & 0x1F) << 8) | l) << 7)
                unicode_mode = False
            elif b == 0xF2:
                raise SCSUError('reserved tag 0xF2 at %d' % (i - 1))
            else:
                need(1); out.append((b << 8) | data[i]); i += 1

    return _join(out)


def _join(out):
    # join, combining UTF-16 surrogate pairs
    s = []
    j = 0
    while j < len(out):
        u = out[j]
        if 0xD800 <= u < 0xDC00 and j + 1 < len(out) and 0xDC00 <= out[j + 1] < 0xE000:
            s.append(chr(0x10000 + ((u - 0xD800) << 10) + (out[j + 1] - 0xDC00))); j += 2
        else:
            s.append(chr(u)); j += 1
    return ''.join(s)


# ---- BOCU-1 (Unicode Technical Note #6); the platform's "SCSU" reader takes it by its BOM FB EE 28
_B_MIN, _B_MIDDLE, _B_RESET = 0x21, 0x90, 0xFF
_TC = (0xFF - _B_MIN + 1) + 20                     # 243 trail values
_R_POS1, _R_NEG1 = 63, -64
_R_POS2 = _R_POS1 + 43 * _TC; _R_NEG2 = _R_NEG1 - 43 * _TC
_R_POS3 = _R_POS2 + 3 * _TC * _TC; _R_NEG3 = _R_NEG2 - 3 * _TC * _TC
_S_POS2 = _B_MIDDLE + _R_POS1 + 1; _S_POS3 = _S_POS2 + 43; _S_POS4 = _S_POS3 + 3
_S_NEG2 = _B_MIDDLE + _R_NEG1; _S_NEG3 = _S_NEG2 - 43
_TRAIL_CTL = [-1, 0, 1, 2, 3, 4, 5, -1, -1, -1, -1, -1, -1, -1, -1, -1,
              6, 7, 8, 9, 10, 11, 12, 13, 14, 15, -1, -1, 16, 17, 18, 19, -1]
BOCU1_BOM = b'\xfb\xee\x28'


def _bprev(c):
    if 0x3040 <= c <= 0x309F: return 0x3070
    if 0x4E00 <= c <= 0x9FA5: return 0x4E00 - _R_NEG2
    if 0xAC00 <= c <= 0xD7A3: return (0xD7A3 + 0xAC00) // 2
    return (c & ~0x7F) + 0x40


def decode_bocu1(data: bytes) -> str:
    prev = 0x40
    out = []
    i, n = 0, len(data)
    while i < n:
        b = data[i]; i += 1
        if b <= 0x20:
            if b != 0x20: prev = 0x40
            out.append(b); continue
        if b == _B_RESET:
            prev = 0x40; continue
        if _S_NEG2 <= b < _S_POS2:
            c = prev + (b - _B_MIDDLE)
        else:
            if b >= _S_NEG2:
                if b < _S_POS3: diff, cnt = (b - _S_POS2) * _TC + _R_POS1 + 1, 1
                elif b < _S_POS4: diff, cnt = (b - _S_POS3) * _TC * _TC + _R_POS2 + 1, 2
                else: diff, cnt = _R_POS3 + 1, 3
            else:
                if b >= _S_NEG3: diff, cnt = (b - _S_NEG2) * _TC + _R_NEG1, 1
                elif b > _B_MIN: diff, cnt = (b - _S_NEG3) * _TC * _TC + _R_NEG2, 2
                else: diff, cnt = -_TC * _TC * _TC + _R_NEG3, 3
            if i + cnt > n: raise SCSUError('BOCU-1: truncated at %d' % i)
            for k in range(cnt, 0, -1):
                t = data[i]; i += 1
                tv = _TRAIL_CTL[t] if t <= 0x20 else t - (_B_MIN - 20)
                if tv < 0: raise SCSUError('BOCU-1: bad trail byte 0x%02X at %d' % (t, i - 1))
                diff += tv * _TC ** (k - 1)
            c = prev + diff
            if not 0 <= c <= 0x10FFFF: raise SCSUError('BOCU-1: code point out of range at %d' % i)
        prev = _bprev(c)
        out.append(c)
    return ''.join(chr(c) for c in out)


def decode_image(data: bytes) -> str:
    """what ПолучитьСтрокуИзДвоичныхДанных(ДД, "SCSU") returns for a module image"""
    return decode_bocu1(data) if data.startswith(BOCU1_BOM) else decode(data)
