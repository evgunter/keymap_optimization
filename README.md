This is a "game" designed to determine how quickly you can press key combinations in which order.
I plan to use a procedure similar to RLHF to build a reward model on sequences of key presses.

The game itself works by displaying a sequence of "chords" (combinations of keys) to press.
You are given some time to practice, and then you start recording by pressing all the keys in the "homerow" (where your fingers naturally sit at rest on the keyboard).
Then, you have to type the sequence several times in a row.
The speed and error rate you have while doing this is recorded, and will be used to build the reward model once that is implemented. 
