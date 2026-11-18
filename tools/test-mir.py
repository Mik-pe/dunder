"""Semantic MIR interpreter used by the SSA, region and expression tests."""
from collections import Counter


MASK64 = (1 << 64) - 1
COLUMNS = ('kind', 'opcode', 'dst', 'a', 'b', 'c', 'imm', 'aux', 'type',
           'place', 'extension', 'effects', 'flags', 'abi', 'reads', 'writes',
           'clobbers', 'mode', 'target')


class CoverageGap(Exception):
    """Semantics outside this deliberately closed interpreter subset."""


class Incomplete(Exception):
    """Fuel exhaustion is never a completed semantic test."""


def signed(value, width=64):
    value &= (1 << width) - 1
    return value - (1 << width) if value >> (width - 1) else value


def quotient(a, b):
    assert b != 0
    return (-1 if (a < 0) != (b < 0) else 1) * (abs(a) // abs(b))


class Interpreter:
    """Mutable physical scalar variables, byte memory, NZCV and explicit CFG.

    W writes zero-extend; X writes preserve all 64 bits. Arithmetic is machine
    arithmetic here. Checked *source* traps are explicit preceding instructions
    and are covered separately by whole-source executions.
    """
    def __init__(self, model, fuel=10000):
        self.model = model
        self.fuel = fuel
        self.steps = 0
        self.variables = {29: 0, 30: 0, 31: 1 << 20}
        self.memory = {}
        self.nzcv = None
        self.coverage = Counter()

    def read(self, variable):
        if variable == -1:
            return 0
        if variable not in self.variables:
            raise CoverageGap(f'read of uninitialized variable {variable}')
        return self.variables[variable]

    def write(self, variable, value, width=64):
        if variable != -1:
            self.variables[variable] = value & ((1 << width) - 1)

    def load(self, address, size):
        if any(address + i not in self.memory for i in range(size)):
            raise CoverageGap(f'uninitialized memory [{address}, {address + size})')
        return sum(self.memory[address + i] << (8 * i) for i in range(size))

    def store(self, address, value, size):
        for i in range(size):
            self.memory[address + i] = (value >> (8 * i)) & 255

    def condition(self, condition):
        if self.nzcv is None:
            raise CoverageGap('NZCV read before definition')
        n, z, c, v = self.nzcv
        choices = (z, not z, c, not c, n, not n, v, not v,
                   c and not z, not c or z, n == v, n != v,
                   not z and n == v, z or n != v, True, True)
        if not 0 <= condition < len(choices):
            raise CoverageGap(f'unknown NZCV condition {condition}')
        return choices[condition]

    def binary(self, op, width):
        mask = (1 << width) - 1
        a = self.read(op['a']) & mask
        b = (self.read(op['b']) if op['b'] >= 0 else op['imm']) & mask
        shift = op['aux'] & 63
        shift_kind = (op['aux'] >> 6) & 3
        # Shifted operands are semantic aux fields, never decoded encodings.
        if op['opcode'] in (1, 2, 8, 9, 10, 11, 12, 13):
            if shift_kind == 0:
                b = (b << shift) & mask
            elif shift_kind == 1:
                b >>= shift
            elif shift_kind == 2:
                b = (signed(b, width) >> shift) & mask
            elif shift:
                b = ((b >> shift) | (b << (width - shift))) & mask
        code = op['opcode']
        if code == 1:
            result = a + b
        elif code == 2:
            result = a - b
        elif code == 3:
            result = a * b
        elif code == 4:
            result = (signed(a, width) * signed(b, width)) >> width
        elif code == 5:
            result = (a * b) >> width
        elif code == 6:
            result = quotient(signed(a, width), signed(b, width)) if b else 0
        elif code == 7:
            result = a // b if b else 0
        elif code in (8, 11):
            result = a & (b if code == 8 else ~b)
        elif code in (9, 12):
            result = a | (b if code == 9 else ~b)
        elif code in (10, 13):
            result = a ^ (b if code == 10 else ~b)
        elif code == 14:
            result = a << (b % width)
        elif code == 15:
            result = a >> (b % width)
        elif code == 16:
            result = signed(a, width) >> (b % width)
        elif code == 17:
            result = self.read(op['c']) + a * b
        elif code == 18:
            result = self.read(op['c']) - a * b
        else:
            raise CoverageGap(f'binary opcode {code}')
        if op['flags'] & 2:
            bits = result & mask
            carry = overflow = False
            if code == 1:
                carry = result > mask
                math = signed(a, width) + signed(b, width)
                overflow = not -(1 << (width - 1)) <= math < 1 << (width - 1)
            elif code == 2:
                carry = a >= b
                math = signed(a, width) - signed(b, width)
                overflow = not -(1 << (width - 1)) <= math < 1 << (width - 1)
            elif code not in (8, 11):
                raise CoverageGap(f'flag-setting binary opcode {code}')
            self.nzcv = (bool(bits >> (width - 1)), bits == 0, carry, overflow)
        return result

    def run(self):
        model = self.model
        labels = dict(model['labels'])
        position = 0
        while position < len(model['order']):
            if self.steps >= self.fuel:
                raise Incomplete(f'fuel={self.fuel}, position={position}')
            self.steps += 1
            op = model['operations'][model['order'][position]]
            kind = op['kind']
            self.coverage[f'{kind}:{op["opcode"]}'] += 1
            next_position = position + 1
            width = 32 if op['type'] == 1 else 64
            if op['mode'] == 2:
                position = next_position
                continue
            if kind == 1:
                self.write(op['dst'], op['imm'], width)
            elif kind == 2:
                self.write(op['dst'], self.read(op['a']), width)
            elif kind == 3:
                value = self.read(op['a'])
                if op['opcode'] == 3:
                    result = (value & ~(65535 << op['aux'])) | op['imm']
                elif op['opcode'] in (1, 2):
                    rotate, top = op['aux'] & 63, op['aux'] >> 6
                    if not (0 <= rotate < width and 0 <= top < width):
                        raise CoverageGap('invalid semantic bitfield bounds')
                    if rotate <= top:
                        bits = top - rotate + 1
                        result = (value >> rotate) & ((1 << bits) - 1)
                    else:
                        bits = top + 1 + width - rotate
                        result = (value & ((1 << (top + 1)) - 1)) << (width - rotate)
                    if op['opcode'] == 1:
                        result = signed(result, bits)
                else:
                    raise CoverageGap(f'unary opcode {op["opcode"]}')
                self.write(op['dst'], result, width)
            elif kind in (4, 5, 16):
                result = self.binary(op, width)
                self.write(op['dst'], result, width)
            elif kind == 6:
                if op['a'] != -1 or op['b'] != -1:
                    raise CoverageGap('general Select operands outside closed CSET subset')
                self.write(op['dst'], int(self.condition(op['imm'])), width)
            elif kind in (7, 8):
                place = model['places'][op['place']]
                assert (place['offset'], place['width']) == (op['imm'], op['aux']), op
                base = op['a'] if kind == 7 else op['b']
                address = self.read(base) + op['imm']
                if kind == 7:
                    value = self.load(address, op['aux'])
                    if op['extension'] == 2:
                        value = signed(value, op['aux'] * 8)
                    self.write(op['dst'], value, width)
                else:
                    self.store(address, self.read(op['a']), op['aux'])
            elif kind == 10:
                stack = self.read(31) - 16
                self.store(stack, self.read(op['a']), 8)
                self.store(stack + 8, 0, 8)
                self.write(31, stack)
            elif kind == 11:
                stack = self.read(31)
                self.write(op['dst'], self.load(stack, 8))
                self.write(31, stack + 16)
            elif kind in (12, 13):
                taken = True
                if kind == 13:
                    if op['opcode'] == 1:
                        taken = self.condition(op['imm'])
                    elif op['opcode'] in (2, 3):
                        taken = (self.read(op['a']) == 0) == (op['opcode'] == 2)
                    else:
                        raise CoverageGap(f'conditional opcode {op["opcode"]}')
                if taken:
                    if op['target'] not in labels or labels[op['target']] < 0:
                        raise CoverageGap(f'external branch {op["target"]}')
                    next_position = labels[op['target']]
            elif kind == 15:
                return self.read(0)
            elif kind == 18:
                pass
            elif kind == 21 and op['abi'] == 1:
                # The documented FP/LR frame pair has explicit memory effects.
                if op['effects'] == 36:
                    stack = self.read(31) - 16
                    self.store(stack, self.read(29), 8)
                    self.store(stack + 8, self.read(30), 8)
                    self.write(31, stack)
                elif op['effects'] == 34:
                    stack = self.read(31)
                    self.write(29, self.load(stack, 8))
                    self.write(30, self.load(stack + 8, 8))
                    self.write(31, stack + 16)
                else:
                    raise CoverageGap(f'ABI frame effects {op["effects"]}')
            else:
                raise CoverageGap(f'kind={kind}, opcode={op["opcode"]}, abi={op["abi"]}')
            position = next_position
        raise CoverageGap('function fell off end without Return')


def decode_model(output):
    lines = iter(output.splitlines())
    assert next(lines) == 'MIR1', output
    count = int(next(lines)); assert 0 < count < 10000, count
    operations = [dict(zip(COLUMNS, (int(next(lines)) for _ in COLUMNS))) for _ in range(count)]
    assert next(lines) == 'PLACES'
    count = int(next(lines)); assert 0 <= count < 10000
    places = [dict(zip(('kind', 'offset', 'width'), (int(next(lines)) for _ in range(3)))) for _ in range(count)]
    assert next(lines) == 'LABELS'
    count = int(next(lines)); assert 0 <= count < 10000
    labels = [(int(next(lines)), int(next(lines))) for _ in range(count)]
    assert next(lines) == 'ORDER'
    count = int(next(lines)); assert count == len(operations)
    order = [int(next(lines)) for _ in range(count)]
    assert sorted(order) == list(range(count))
    assert next(lines) == 'END' and next(lines, None) is None
    return dict(operations=operations, places=places, labels=labels, order=order)
