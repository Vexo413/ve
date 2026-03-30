mod chunk;
mod constants;
mod position;
mod render;
#[cfg(test)]
mod tests;
mod world;

fn main() {
    render::start();
}
