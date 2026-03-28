mod chunk;
mod constants;
mod position;
#[cfg(test)]
mod test;
mod wgpu;
mod world;

fn main() {
    wgpu::start();
}
