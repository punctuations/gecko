from gecko import sandbox

report = sandbox.run("total = 0\nfor i in range(10):\n    total += i\nprint(total)")
print("sandbox printed:", report)

try:
    sandbox.run("while True:\n    pass", 5000)
except SandboxError as e:
    print("stopped runaway loop:", e)

try:
    sandbox.run("while True:\n    pass", 0, 0, 20)
except SandboxError as e:
    print("stopped slow run:", e)

try:
    sandbox.run("x = 0\nwhile True:\n    x = [x, x]", 10000000, 400)
except SandboxError as e:
    print("stopped memory hog:", e)

try:
    sandbox.run("print(1 / 0)")
except SandboxError as e:
    print("sandboxed error:", e)

secret = "host only"
sandbox.run("secret = 'sandbox copy'\nprint(secret)")
print("host secret still:", secret)
