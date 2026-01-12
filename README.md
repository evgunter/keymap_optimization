# goal

The goal of this project is to build a keymap for a chording keyboard which is suited to my hand size (small) and as efficient as possible.
(A chording keyboard is a keyboard where multiple keys are pressed at once; each chord may type multiple letters.)

To collect data, there is a "game" to determine how quickly you can press key combinations in which order.
These data are used to build a reward model for sequences of key presses.
This reward model will then be used along with a tokenizer trained on a diverse corpus of my own text (including code, $\LaTeX$, messages, and writing) to generate a keymap which is efficient for all my use cases.

(A previous version of this project used a hand-designed reward model and optimized the keymap with simulated annealing. Unfortunately, I was unable to hand-design a reward model which captured my preferences accurately when optimized against. My hope is that a trained reward model will hold up better, and I'm optimistic since RLHF works in a similar way.)

## "game" details

The game itself works by displaying a sequence of "chords" (combinations of keys) to press.
You are given some time to practice, and then you start recording by pressing all the keys in the "homerow" (where your fingers naturally sit at rest on the keyboard).
Then, you have to type the sequence several times in a row.
The speed and error rate you have while doing this is recorded, and are used to build the reward model.
