"""Fit exponential decay-heat groups to 0.066*t^-0.2 (infinite operation, Todreas & Kazimi)."""
import numpy as np
from scipy.optimize import nnls
lams = np.logspace(-0.3, -7.5, 9)
t = np.logspace(0, 7, 200)
target = 0.066 * t ** -0.2
A = np.exp(-np.outer(t, lams))
w = 1 / target
f, _ = nnls(A * w[:, None], target * w)
fit = A @ f
print("sum f =", f.sum())
for tt in [1, 10, 100, 1000, 3600, 86400, 604800]:
    print(tt, 0.066 * tt ** -0.2, (f * np.exp(-lams * tt)).sum())
print("max rel err", np.max(np.abs(fit / target - 1)))
for l, ff in zip(lams, f):
    print(f"    ({ff:.6e}, {l:.6e}),")
