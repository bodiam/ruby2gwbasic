#!/usr/bin/env python3
"""Optional output comparison using Ruby and PC-BASIC (pip install pcbasic)."""

import io
import pathlib
import subprocess

import pcbasic


ROOT = pathlib.Path(__file__).resolve().parent.parent
BINARY = ROOT / "target" / "debug" / "ruby2gwbasic"


def verify(name, source):
    listing = subprocess.check_output([BINARY, "-"], input=source, timeout=10)
    expected = subprocess.check_output(["ruby", "-"], input=source, timeout=10)
    output = io.BytesIO()
    with pcbasic.Session(input_streams=None, output_streams=output) as session:
        session.execute(listing)
        session.execute("RUN")
    actual = output.getvalue().replace(b"\r\n", b"\n")
    assert actual == expected, (
        f"{name}: output mismatch\nRuby: {expected!r}\nBASIC: {actual!r}\n"
        f"Listing:\n{listing.decode('ascii')}"
    )
    print(f"PASS {name}")


subprocess.run(["cargo", "build", "--quiet"], cwd=ROOT, check=True)

for example in sorted((ROOT / "examples").glob("*.rb")):
    verify(example.name, example.read_bytes())

verify("division and modulo with both signs", b"""
a = -7
while a <= 7
  b = -3
  while b <= 3
    if b != 0
      puts a / b
      puts a % b
    end
    b += 1
  end
  a += 1
end
""")

verify("short-circuit conditions and negation", b"""
x = 0
if x != 0 && 10 / x > 1
  puts "wrong"
elsif x == 0 || 10 / x > 1
  puts "safe"
end
if !(x != 0)
  puts "negation"
end
if false && 1 / 0 == 1
  puts "wrong"
end
if true || 1 / 0 == 1
  puts "literal"
end
""")

verify("strings, quotes, and newline handling", br"""
greeting = "Hello"
puts greeting + ", world!"
puts "say \"hello\" # this is a string"
print 'one', 'two'
puts
puts "already ends in a newline\n"
puts "two newlines\n\n"
puts ""
puts()
print()
""")

verify("precedence, reassignment, and branch initialization", b"""
x = -5 / 2
x *= 3
x %= 4
if x >= 0 && x < 4
  result = "in range"
else
  result = "wrong"
end
puts result
puts -5 / 2
puts -(5 / 2)
puts 2 + 3 * 4
puts (2 + 3) * 4
puts 10 - 3 - 2
puts +7
if "abc" < "abd"
  puts "string comparison"
end
""")
