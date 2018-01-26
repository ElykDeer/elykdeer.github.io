---
layout: sections
title: Projects
permalink: /projects/
sections:
  Sapling Studios: "#sapling-studios"
  Other Projects: "#other-projects"
---

{% include projects.md %}

## Sapling Studios
Sapling Studios is the pen-name that I use to publish the projects that I intend to provide support for an extended period of time on.  Currently, Sapling Studios EMA, PEP, and bananaScript.

EMA is a framework with which to make modular time-based 2D games, while PEP is my first attempt making anything with EMA.  The development of those projects was at their height in the summer of 2017, but since then I have not had nearly as much time as I would like to have to work on them.

[]()
{: style="height:2.5vh;" }

## CSAW Challenges
### [bananaScript](https://github.com/isislab/CSAW-CTF-2017-Quals/tree/master/rev/bananascript)
The first Capture The Flag Challenge I wrote was bananaScript, an esotaric programming language that I first designed for CSAW Quals.  bananaScript is a programming language in which all actions, text-encodings, and everything else that it uses are formed with the word "bananas" in different capitalizations.  To help you understand, here's an example script that prints "Loopy" in an infinite loop:

~~~
banANAS baNanas BaNAnAs BANanaS BANanaS BANanas BAnaNAS
bananas banANAS
bananAS banANAS baNanas bANaNAS BanaNAS
bananAs bananAS banANAS
~~~

For the challenge, one had to reverse the bananaScript interpreter that I wrote and first teach themselves bananaScript, and then reverse a complex banana-script that asked four questions, took four inputs, added them all together, and xor'ed them against the flag.  You can read a writeup about how one team did that, [here](https://github.com/ShellCollectingClub/csaw2017/tree/master/bananascript).

bananaScripts will be maintained [here](https://github.com/KyleMiles/bananaScript) for any desired future use.

### [Super Banana Babbel](https://github.com/KyleMiles/bananaScript/HSF)
Super Banana Babbel was another challenge I wrote for [HSF](https://csaw.engineering.nyu.edu/hsf) that used bananaScript.  This time, 136 different interpreters were given with 136 different banana-scripts.  Each script only ran properly with one of the interpreters, and printed out the old and new testament (which served as an arbitrarily large text in the public domain), except with one line replaced with "The \_th character of the flag is \_", which was trivial to use regular expressions to parse out.

### [Are You Alive](https://github.com/isislab/CSAW-CTF-2017-Finals/blob/master/Misc/areYouAlive.txt)
This wasn't much in terms of a challenge, but it is my favorite steg problem.

### [thoroughlyStripped](https://github.com/isislab/CSAW-CTF-2017-Finals/tree/master/Forensics/thoroughlyStripped)
Written for CSAW 2017 Finals, this was a simple binary program that printed the flag, except I had removed all null-bytes from the file.  Once the pattern was recognized, it was simple to create a signature for all the functions being called (printf, printl, printa, printg, printOpenBracket, etc).  The difficult part was finding the main function and pulling out the order in which all those functions were called.  The function pointers could be found, and then the flag could be extracted based on the magnitude of the pointer and the order of the function signature.

### [Humm_sCh-t](https://github.com/isislab/CSAW-CTF-2017-Finals/tree/master/Pwn/Humm_sCh-t)
Humm_sCh-t ("Hummus-Chat") is a fully-featured terminal-based chat service that used ncurses for the GUI. The theme of this challenge is that you connect to "Elyk's personal dev server" and are the beta tester for his "hit new language, Humm_s." The challenge in this for most teams ended up being in the riddle-like language that the challenge presented itself with.  In the end, the program had the features of being able to list Humm_s commands in descending order, the first three of which were implemented in the server (so the last three that printed out).  After deducing the packet structure from how the binary constructed them, teams could send commands at the server and figure out what they do.  One of the commands allowed for remote execution.

## Other Projects
Currently in the works include an update to bananaScript, my ScrapBook, my Schoolwork, and I am work on hacking Bluetooth (will update later with more info). I am also exploring distributed networks, and code snippets for that will start to filter out of closed-source over some time. One thing I am looking into right now is simulating a simplified version of Tor with some VMs and a lot of code to learn how distributed networks and decision making works at scale.

In the relatively near future I am planning to go back to [SnakeOS](https://github.com/KyleMiles/SnakeOS), and make more flushed-out operating system.
