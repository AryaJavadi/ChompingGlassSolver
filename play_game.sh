#!/bin/bash
# Interactive Chomping Glass game simulator

echo "🎮 Chomping Glass - Interactive Game"
echo "======================================"
echo ""

# Move 1: You play (1,2) - the only winning opener
echo "Move 1 - YOU play (1,2):"
cargo run -p cli --release -- suggest --state="0,-1,-1,-1,-1,-1,-1,-1" 2>/dev/null
echo ""
read -p "Press Enter to see AI's response options..."
echo ""

# AI could play (2,1), (1,3), (3,1), (4,1), or (5,1) - let's test (2,1)
echo "Move 2 - AI plays (2,1) [chomp rows 0-1 of col 0]:"
cargo run -p cli --release -- suggest --state="1,-1,-1,-1,-1,-1,-1,-1" 2>/dev/null
echo ""
read -p "Press Enter to continue..."
echo ""

# You must play (1,5) to maintain winning
echo "Move 3 - YOU play (1,5):"
cargo run -p cli --release -- suggest --state="1,-1,-1,-1,0,-1,-1,-1" 2>/dev/null
echo ""
read -p "Press Enter to see AI's next move..."
echo ""

# AI plays (3,4)
echo "Move 4 - AI plays (3,4):"
cargo run -p cli --release -- suggest --state="1,-1,-1,2,0,-1,-1,-1" 2>/dev/null
echo ""
read -p "Press Enter to continue..."
echo ""

# Your next winning move
echo "Move 5 - YOU play one of the winning moves:"
cargo run -p cli --release -- suggest --state="1,-1,-1,2,0,-1,-1,-1" 2>/dev/null
echo ""
echo "✅ Game demo complete! The solver guides you to victory."

