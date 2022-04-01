# Ape Pallet

This is a substrate pallet implementation where users can mint, buy/sell, set price, and transfer Ape NFTs. Ape in degen frens.

# Instructions
1. First make sure you have rust installed. 
2. `git clone git@github.com:h3lio5/ape_pallet_substrate.git`
3. "cd" into the root of the ape_pallet_substrate repository. 
4. Run `cargo build --release`. Wait for some time for the program to compile (it may take a while).
5. Run `./target/release/node-template --dev --tmp`. If everything went well, you will see a local node running on port `127.0.0.1:9944` and new blocks being finalized.
6. For an interactive experience, head over to the polkadot.js.org website and set the local node as your environment (you can find it in the top left corner of the webpage and selecting "Development" tab. 
7. Voila! you can now interact with whichever pallet you want. Click on the "Developer" toggle button on the top of the page, and you can find "Extrinsics" (for function calls) and "Chain State" (to query the storage). 
