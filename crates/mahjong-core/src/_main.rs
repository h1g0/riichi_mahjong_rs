use rand::seq::SliceRandom;

mod tile;
use tile::*;

mod hand;
use hand::*;

mod hand_info;
use hand_info::*;

fn main() {
    let mut rng = rand::thread_rng();
    let mut tiles: Vec<Tile> = Vec::new();
    for _ in 0..4 {
        for i in 0..Tile::LEN {
            tiles.push(Tile::new(i as TileType));
        }
    }
    tiles.shuffle(&mut rng);
    let mut hand_vec: Vec<Tile> = Vec::new();
    for _ in 0..13 {
        hand_vec.push(tiles.pop().unwrap());
    }
    hand_vec.sort();
    let hand = Hand::new(hand_vec, tiles.pop());
    println!("{}", hand.to_short_string());
}
