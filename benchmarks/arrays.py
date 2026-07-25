from gecko import array

n = 200000
xs = array(range(n), dtype="f64")
ys = array(range(n), dtype="f64")
a = array(range(n), dtype="i64")
b = array(range(n), dtype="i64")

t = 0.0
for _ in range(60):
    t = (xs * ys + xs)[n - 1]
u = 0
for _ in range(60):
    u = (a + b)[n - 1]
print(t, u, xs.sum(), a.sum())
