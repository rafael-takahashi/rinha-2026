# Rinha 2026

## Algoritmos e Estrutura de Dados

- K-Nearest Neighbors (KNN) - exato, K=5
- KD-Tree - imutável, 14 dimensões, bucket size 32
- Distância Euclidiana (ao quadrado, via `kiddo::SquaredEuclidean`)

## Redes

- HTTP sobre Unix domain sockets (variável de ambiente SOCKET_PATH)
- Nginx com load balancing round-robin entre duas instâncias da API
- Porta 9999 exposta externamente

## Tecnologias e Dependências

- Rust
- Tokio - runtime assíncrono
- Axum - framework HTTP
- kiddo - KD-Tree imutável
- rkyv - desserialização zero-copy (KD-Tree serializada no build, desserializada diretamente do mmap)
- memmap2 - I/O com arquivo mapeado em memória
- sonic-rs - parsing de JSON de alta performance
- serde / serde_json - serialização
- flate2 - descompressão gzip (dataset de referência)
- Nginx
