# Interactive Article Showcase

::: infobox
title: Showcase
image: media/sample.svg
Purpose: Demonstrate every embed type
Formats: Markdown, LaTeX, code, sheets, maps
Languages: Python, C, C++, Rust, JavaScript, Java
Source: [Example site](https://example.com)
:::

This article demonstrates every element that a **WikiForge** page can contain. Each section below shows one feature, and the Markdown that produced it can be inspected in the editor or in the `pages/showcase.md` file. Return to the [[main-page|main page]] at any time; links to other articles are blue, while links to outside websites, such as [Wikipedia](https://en.wikipedia.org/), are green.[^1]

## Links and footnotes

Internal links use double brackets, as in [[main-page]] or [[main-page|a custom label]]. External links use the usual Markdown form, for example [the Rust language](https://www.rust-lang.org/). Footnotes are written as a marker in the text[^2] and a definition elsewhere in the file; they are numbered in order of appearance and collected under *Notes* at the end of the article. A marker may be reused[^1] and every reference links back to its place in the text.

## Mathematics with LaTeX

Inline mathematics is wrapped in single dollar signs: Euler's identity is $e^{i\pi} + 1 = 0$, and the quadratic formula gives $x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}$. Display mathematics gets its own centered line:

$$
\int_{-\infty}^{\infty} e^{-x^2}\,dx = \sqrt{\pi}
$$

$$
\nabla \times \mathbf{B} = \mu_0 \mathbf{J} + \mu_0 \varepsilon_0 \frac{\partial \mathbf{E}}{\partial t}
$$

## Tables

### Static tables

Ordinary Markdown tables are rendered as non-interactive tables in the Wikipedia "wikitable" style.

| Planet  | Diameter (km) | Moons | Notes                |
|:--------|--------------:|------:|:---------------------|
| Mercury |         4,879 |     0 | Smallest planet      |
| Venus   |        12,104 |     0 | Hottest surface      |
| Earth   |        12,742 |     1 | Only known life      |
| Mars    |         6,779 |     2 | The red planet       |

### Interactive spreadsheet

The table below behaves like a small spreadsheet. Click a cell and edit it; cells that start with `=` are formulas. Use `;` to separate function arguments so that cells never need CSV quoting.

```sheet
Item,Qty,Price,Total
Apples,4,0.5,=B2*C2
Pears,6,0.75,=B3*C3
Cherries,20,0.2,=B4*C4
Total,=SUM(B2:B4),,=SUM(D2:D4)
Average price,,=ROUND(AVERAGE(C2:C4);2),
Big order?,,,=IF(D5>10;"yes";"no")
```

## Graphs with Desmos

Drag the sliders or edit the expressions. One expression is written per line in the `desmos` block.

```desmos height=420
bounds: -10, 10, -6, 6
y=a\sin\left(bx\right)
a=2
b=1
y=\frac{1}{2}x^2-3
```

## Code windows

Every code window can be edited and run in place. JavaScript and Python run in your browser; C, C++, Rust and Java are compiled and run by public online services, so they need an internet connection and may be slower.

### JavaScript

```javascript
const squares = Array.from({ length: 10 }, (_, i) => i * i);
console.log("Squares:", squares.join(", "));
console.log("Sum:", squares.reduce((a, b) => a + b, 0));
```

### Python

```python
def primes(n):
    sieve = [True] * (n + 1)
    sieve[0:2] = [False, False]
    for i in range(2, int(n ** 0.5) + 1):
        if sieve[i]:
            for j in range(i * i, n + 1, i):
                sieve[j] = False
    return [i for i, p in enumerate(sieve) if p]

print(primes(60))
```

### C

```c
#include <stdio.h>

int main(void) {
    long a = 0, b = 1;
    for (int i = 0; i < 15; i++) {
        printf("%ld ", a);
        long t = a + b; a = b; b = t;
    }
    printf("\n");
    return 0;
}
```

### C++

```cpp
#include <iostream>
#include <vector>
#include <algorithm>

int main() {
    std::vector<int> v = {5, 3, 9, 1, 7};
    std::sort(v.begin(), v.end());
    for (int x : v) std::cout << x << " ";
    std::cout << "\n";
}
```

### Rust

```rust
fn main() {
    let words = ["alpha", "beta", "gamma"];
    for (i, w) in words.iter().enumerate() {
        println!("{}: {}", i + 1, w.to_uppercase());
    }
}
```

### Java

```java
public class Main {
    public static void main(String[] args) {
        StringBuilder sb = new StringBuilder();
        for (int i = 1; i <= 5; i++) sb.append(i * i).append(' ');
        System.out.println(sb.toString().trim());
    }
}
```

## Code windows with a canvas

A code window can include a drawing canvas. JavaScript and Python call the drawing functions directly. C, C++, Rust and Java print commands that begin with `@`, one per line, and the page draws them. Pixel commands use a top-left origin, while turtle commands use a centre origin with the y axis pointing up.

| Command                                | Meaning                                  |
|:---------------------------------------|:-----------------------------------------|
| `rect(x, y, w, h, color)`              | Filled rectangle                         |
| `ellipse(cx, cy, rx, ry, color)`       | Filled ellipse                           |
| `circle(cx, cy, r, color)`             | Filled circle                            |
| `line(x1, y1, x2, y2, color)`          | Line segment                             |
| `text(x, y, string)`                   | Text                                     |
| `forward(n)`, `left(a)`, `right(a)`    | Turtle movement and turning              |
| `penup()`, `pendown()`, `goto(x, y)`   | Turtle pen control                       |
| `color(c)`, `pensize(w)`, `clear()`    | Turtle color, line width, clear canvas   |

### JavaScript with canvas

```javascript canvas
rect(20, 20, 120, 80, "#3366cc");
ellipse(260, 70, 70, 40, "#dd5522");
text(160, 40, "Hello, canvas!");
color("#228833");
pensize(2);
for (let i = 0; i < 36; i++) {
  forward(120);
  left(170);
}
```

### Python with canvas

```python canvas
import turtle

rect(20, 20, 120, 80, "#3366cc")
ellipse(260, 70, 70, 40, "#dd5522")
text(160, 40, "Hello, canvas!")
turtle.color("#228833")
turtle.pensize(2)
for i in range(36):
    turtle.forward(120)
    turtle.left(170)
```

### C with canvas

```c canvas
#include <stdio.h>

int main(void) {
    printf("@rect 20 20 120 80 #3366cc\n");
    printf("@ellipse 260 70 70 40 #dd5522\n");
    printf("@color #228833\n@pensize 2\n");
    for (int i = 0; i < 36; i++) {
        printf("@forward 120\n@left 170\n");
    }
    return 0;
}
```

### C++ with canvas

```cpp canvas
#include <iostream>

int main() {
    std::cout << "@rect 20 20 120 80 #3366cc\n";
    std::cout << "@ellipse 260 70 70 40 #dd5522\n";
    std::cout << "@color #228833\n@pensize 2\n";
    for (int i = 0; i < 36; i++) {
        std::cout << "@forward 120\n@left 170\n";
    }
}
```

### Rust with canvas

```rust canvas
fn main() {
    println!("@rect 20 20 120 80 #3366cc");
    println!("@ellipse 260 70 70 40 #dd5522");
    println!("@color #228833");
    println!("@pensize 2");
    for _ in 0..36 {
        println!("@forward 120");
        println!("@left 170");
    }
}
```

### Java with canvas

```java canvas
public class Main {
    public static void main(String[] args) {
        System.out.println("@rect 20 20 120 80 #3366cc");
        System.out.println("@ellipse 260 70 70 40 #dd5522");
        System.out.println("@color #228833");
        System.out.println("@pensize 2");
        for (int i = 0; i < 36; i++) {
            System.out.println("@forward 120");
            System.out.println("@left 170");
        }
    }
}
```

## Interactive maps

Drag to pan, use the buttons to zoom, and click a marker for its label.

```map height=380
center: 40.7580, -73.9855
zoom: 13
marker: 40.7580, -73.9855 | Times Square
marker: 40.7484, -73.9857 | Empire State Building
circle: 40.7527, -73.9772 | 600
line: 40.7580,-73.9855; 40.7484,-73.9857 | A short walk
```

## Interactive 3D models

Models in the glTF binary format (`.glb`) can be stored in the `media` folder or loaded from a URL. Drag to rotate and scroll to zoom.

```model src="https://modelviewer.dev/shared-assets/models/Astronaut.glb" alt="A 3D astronaut" height=420
```

## Images, GIFs and videos

::: figure src="media/sample.svg" caption="A still image from the *media* folder, floated to the right." align=right width=320
:::

Images are inserted with a figure block. A figure can float to the left or right of the text, as this one does, or sit centered on its own line. Captions support the same inline formatting as the rest of the article, including links and mathematics such as $a^2 + b^2 = c^2$.

::: figure src="media/sample.gif" caption="An animated GIF." align=left width=160
:::

Animated GIFs are treated exactly like other images. Videos work the same way and receive playback controls automatically; external video links and YouTube links are supported too.

::: figure src="https://interactive-examples.mdn.mozilla.net/media/cc0-videos/flower.mp4" caption="A video, centered. Replace the address with a file in the media folder." align=center width=480
:::

## Styled text and callouts

::: text font=Georgia size=20 color=#7a1f1f align=center
Text blocks can use their own font, size, color and alignment.
:::

::: note
This is a *note* callout. Blocks named `tip` and `warning` are also available.
:::

::: warning
Code that runs on remote services is sent to those services. Do not put secrets in it.
:::

## Further reading

- [Rust documentation](https://doc.rust-lang.org/)
- [KaTeX supported functions](https://katex.org/docs/supported.html)
- Other articles on this site, such as [[main-page]]

[^1]: A footnote can contain *formatting*, [links](https://en.wikipedia.org/) and mathematics such as $\pi \approx 3.14159$.
[^2]: Footnote definitions can appear anywhere in the file; they are always listed at the end of the rendered article.
