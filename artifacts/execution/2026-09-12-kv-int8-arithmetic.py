import hashlib, json, math, random, struct
from pathlib import Path


def f32(value):
    return struct.unpack('>f', struct.pack('>f', value))[0]


def unpack(word):
    return struct.unpack('>f', word.to_bytes(4, 'big'))[0]


def encode(values):
    peak = max(abs(value) for value in values)
    codes = []
    reconstructed = []
    for value in values:
        ratio = 0.0 if peak == 0.0 else value / peak * 127.0
        code = int(math.copysign(math.floor(abs(ratio) + 0.5), ratio))
        assert -127 <= code <= 127
        codes.append(code)
        reconstructed.append(0.0 if code == 0 else f32(peak * code / 127.0))
    return peak, codes, reconstructed


def main():
    randomizer = random.Random(728193)
    groups = [[127.0, 63.5, -63.5, 0.0], [unpack(0x7f7fffff), 1.0, -unpack(0x7f7fffff)],
              [unpack(1), -unpack(1), -0.0], [0.0, -0.0]]
    for _ in range(2048):
        row = []
        for _ in range(randomizer.randint(1, 128)):
            while True:
                value = unpack(randomizer.getrandbits(32))
                if math.isfinite(value):
                    break
            row.append(value)
        groups.append(row)
    values_checked = zeroed = 0
    for row in groups:
        peak, codes, restored = encode(row)
        assert peak == 0.0 or max(abs(code) for code in codes) == 127
        for x, y in zip(row, restored):
            assert math.isfinite(y) and abs(y) <= peak
            tolerance = peak / 254.0 + abs(y) * 2**-23 + unpack(1)
            assert abs(y - x) <= tolerance
            values_checked += 1
            zeroed += int(x != 0.0 and y == 0.0)
    assert encode(groups[0])[1] == [127, 64, -64, 0]
    assert encode(groups[1])[2][1] == 0.0
    # Independent closed-form uniform-attention decoder control: embeddings
    # [1,0] then [0,-1], V rows [127,0],[0.25,0], O rows [0,1],[0,0].
    epsilon = 0.00001
    normalized = f32(1.0 / math.sqrt(0.5 + epsilon))
    kv = [f32(127.0 * normalized), f32(0.25 * normalized)]
    peak, codes, quantized = encode(kv)
    raw_x = f32(kv[1] / 2.0)
    quantized_x = f32(quantized[1] / 2.0)
    raw_logit = f32(raw_x / math.sqrt((raw_x * raw_x + 1.0) / 2.0 + epsilon))
    quantized_logit = f32(quantized_x / math.sqrt((quantized_x * quantized_x + 1.0) / 2.0 + epsilon))
    assert raw_logit > 0.0 and quantized_logit == 0.0
    here = Path(__file__)
    root = here.parents[2]
    implementation = root / 'crates/fa-reference/src/action/consequence/activation/tensor/kv/model/quantized.rs'
    result = {'status': 'passed_python_arithmetic_only', 'rust_compiled': False, 'rust_tests_executed': False,
              'groups': len(groups), 'values_checked': values_checked, 'nonzero_values_quantized_to_zero': zeroed,
              'rare_signal_control': {'source_values': kv, 'codes': codes, 'raw_next_token': 1,
                                      'quantized_next_token': 0, 'raw_logit': raw_logit, 'quantized_logit': quantized_logit},
              'script_sha256': hashlib.sha256(here.read_bytes()).hexdigest(),
              'quantized_source_sha256': hashlib.sha256(implementation.read_bytes()).hexdigest(),
              'scope': 'Synthetic arithmetic invariants and closed-form choice reversal, not Rust, trained-model, or runtime qualification.'}
    destination = here.with_suffix('.json')
    destination.write_text(json.dumps(result, indent=2, sort_keys=True) + '\n')
    print(destination.read_text())


if __name__ == '__main__':
    main()
