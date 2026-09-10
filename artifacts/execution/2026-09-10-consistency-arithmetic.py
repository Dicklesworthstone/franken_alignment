"""Operator-only cross-check of the limb algorithm; does not execute Rust."""
from fractions import Fraction
from pathlib import Path
import hashlib, json, random
WORDS, MASK, SCALE, SEED = 129, (1 << 64) - 1, 65536, 410913

def mul(words, factor):
    result, carry = [], 0
    for word in words:
        value = word * factor + carry
        result.append(value & MASK)
        carry = value >> 64
    assert carry == 0
    return result

def as_int(words):
    return sum(word << (64 * i) for i, word in enumerate(words))

def path_check(path, alpha):
    n = [1] + [0] * (WORDS - 1)
    d = n.copy()
    exact, first, expected_first = Fraction(1), None, None
    for step, (p, q, event) in enumerate(path, 1):
        fn, fd = (q, p) if event else (SCALE-q, SCALE-p)
        n, d = mul(n, fn), mul(d, fd)
        exact *= Fraction(fn, fd)
        assert Fraction(as_int(n), as_int(d)) == exact
        crossed = tuple(reversed(mul(n, alpha.numerator))) >= tuple(reversed(mul(d, alpha.denominator)))
        if crossed and first is None: first = step
        if exact >= 1 / alpha and expected_first is None: expected_first = step
        assert first == expected_first
    return len(path)

rng, steps = random.Random(SEED), 0
for _ in range(64):
    denominator = rng.randrange(2, MASK + 1)
    alpha = Fraction(rng.randrange(1, denominator), denominator)
    steps += path_check([(rng.randrange(1, SCALE), rng.randrange(1, SCALE), bool(rng.getrandbits(1))) for _ in range(64)], alpha)
for event in [False, True]:
    steps += path_check([(1, 65535, event)] * 512, Fraction(MASK - 1, MASK))
steps += path_check([(16384, 49152, True), (16384, 49152, True), (16384, 49152, False)], Fraction(1,4))
null_crossing = Fraction(0)
for mask in range(256):
    probability, e, crossed = Fraction(1), Fraction(1), False
    for i in range(8):
        event = bool(mask & (1 << i))
        probability *= Fraction(1 if event else 3, 4)
        e *= Fraction(3,1) if event else Fraction(1,3)
        crossed |= e >= 4
    if crossed: null_crossing += probability
assert 0 < null_crossing <= Fraction(1,4)
result = dict(scope="Python limb port against Fraction; NOT Rust execution or calibration evidence", seed=SEED, random_paths=64, exact_step_checks=steps, maximum_steps=512, exhaustive_null_histories=256, null_crossing_probability=str(null_crossing), declared_alpha="1/4", mismatches=0, script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest())
Path(__file__).with_suffix('.json').write_text(json.dumps(result, indent=2)+'\n')
print(json.dumps(result, indent=2))
