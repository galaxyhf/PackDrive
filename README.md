# PackDrive

Aplicativo desktop leve para organizar e copiar arquivos de atendimentos para uma pasta local sincronizada pelo Google Drive para computador.

O PackDrive não utiliza a API do Google Drive. Todas as operações acontecem diretamente no sistema de arquivos, e o Google Drive para computador continua responsável pela sincronização com a nuvem.

O aplicativo funciona no Windows e no macOS. O pacote deve ser compilado para a arquitetura Intel ou Apple Silicon do Mac de destino.

## Sumário

- [Principais recursos](#principais-recursos)
- [Como funciona](#como-funciona)
- [Tecnologias](#tecnologias)
- [Requisitos](#requisitos)
- [Instalação e desenvolvimento](#instalação-e-desenvolvimento)
- [Configuração inicial](#configuração-inicial)
- [Comportamento das transferências](#comportamento-das-transferências)
- [Estrutura do projeto](#estrutura-do-projeto)
- [Comandos disponíveis](#comandos-disponíveis)
- [Validação e testes](#validação-e-testes)
- [Gerando o instalador](#gerando-o-instalador)
- [Privacidade e segurança](#privacidade-e-segurança)
- [Solução de problemas](#solução-de-problemas)
- [Limitações conhecidas](#limitações-conhecidas)

## Principais recursos

- Detecção automática do Google Drive no Windows e em `~/Library/CloudStorage` no macOS.
- Seleção manual do Google Drive quando a detecção automática não encontra o local correto.
- Envio rápido para pastas no formato `[NUMERO_DO_ATENDIMENTO]`.
- Seleção de arquivos, pastas ou uma combinação dos dois.
- Suporte a arrastar e soltar vários itens sobre a janela.
- Navegador manual de pastas dentro do destino configurado.
- Criação de pastas com nomes compatíveis entre Windows e macOS.
- Preservação da estrutura de diretórios, incluindo subpastas vazias.
- Cópia em blocos pelo backend Rust, sem carregar arquivos inteiros na memória.
- Progresso geral e do arquivo atual, velocidade, quantidade de itens e tempo estimado.
- Cancelamento seguro após o bloco de escrita atual.
- Arquivos temporários `.uploading` para evitar a sincronização de conteúdo incompleto.
- Políticas para duplicados: renomear, substituir, ignorar ou perguntar.
- Histórico local com abertura do destino, repetição do envio e cópia do caminho.
- Preferências e histórico persistidos localmente com o plugin Store do Tauri.

## Como funciona

### Envio rápido

1. O PackDrive valida o local configurado para o Google Drive.
2. O usuário escolhe uma única pasta de destino na lista de pastas do Drive.
3. Quando disponível, o destino inicial é `Drives compartilhados/CONTROLE DE PROPRIEDADES DE TERCEIROS/IMPLANTAÇÃO/PACK`.
4. O usuário informa somente o número do atendimento.
5. O aplicativo apresenta a pasta que será criada, por exemplo `[1234567890]`.
6. O usuário seleciona ou arrasta os arquivos e pastas.
7. O backend cria a pasta no destino escolhido e copia os itens.
8. Ao concluir, o envio é registrado no histórico e pode ser aberto no Explorador de Arquivos ou Finder.

Exemplo:

```text
Pasta selecionada:
E:\Google Drive\Drives compartilhados\CONTROLE DE PROPRIEDADES DE TERCEIROS\IMPLANTAÇÃO\PACK

Atendimento:
1234567890

Destino:
E:\Google Drive\Drives compartilhados\CONTROLE DE PROPRIEDADES DE TERCEIROS\IMPLANTAÇÃO\PACK\[1234567890]
```

Se a pasta do atendimento já existir, ela não é apagada. Os novos itens são adicionados de acordo com a política de duplicados configurada.

### Navegação manual

Em **Navegar no Drive**, é possível:

- entrar e voltar entre subpastas;
- atualizar a listagem;
- exibir ou ocultar arquivos;
- criar e selecionar uma nova pasta;
- abrir o destino no Explorador;
- enviar itens diretamente para a pasta escolhida.

## Tecnologias

| Camada | Tecnologia |
| --- | --- |
| Desktop | Tauri 2 |
| Interface | React 19, TypeScript e Vite |
| Estilos | Tailwind CSS 4 e CSS |
| Ícones | Lucide React |
| Backend nativo | Rust |
| Seleção de arquivos | Plugin Dialog do Tauri |
| Preferências e histórico | Plugin Store do Tauri |
| Abertura de pastas | Plugin Opener do Tauri |
| Percurso de diretórios | `walkdir` |

## Requisitos

### Para utilizar

No Windows:

- Windows 10 ou 11;
- Google Drive para computador instalado e conectado;
- acesso de leitura às origens selecionadas;
- acesso de escrita à pasta de destino.

No macOS:

- uma versão do macOS compatível com o Tauri 2 e com a versão instalada do Google Drive para computador;
- Google Drive para computador instalado e conectado;
- acesso de leitura às origens selecionadas;
- acesso de escrita à pasta de destino;
- quando solicitado pelo macOS, permissão para acessar arquivos e pastas.

### Para desenvolver

- Node.js `20.19` ou superior, ou `22.12` ou superior;
- npm;
- Rust estável com Cargo;
- dependências de desenvolvimento do [Tauri 2](https://v2.tauri.app/start/prerequisites/).

No Windows, também são necessários o Microsoft C++ Build Tools com a carga de trabalho **Desenvolvimento para Desktop com C++** e o Microsoft Edge WebView2. No macOS, instale as ferramentas de linha de comando do Xcode:

```bash
xcode-select --install
```

O Google Drive para computador não é necessário para compilar o projeto, mas é necessário para testar o fluxo real de envio.

## Instalação e desenvolvimento

Clone ou abra o repositório e instale as dependências:

```bash
npm install
```

Inicie o aplicativo em modo de desenvolvimento:

```bash
npm run tauri dev
```

Esse comando inicia o Vite na porta `1420`, compila o backend Rust e abre a janela nativa do Tauri.

Para executar apenas a interface no navegador:

```bash
npm run dev
```

Algumas funções dependem do runtime do Tauri e não estarão disponíveis ao executar somente o Vite.

## Configuração inicial

Na primeira execução:

1. O PackDrive procura automaticamente a raiz montada pelo Google Drive para computador.
2. No Windows, reconhece caminhos como `E:\Google Drive`; no macOS, reconhece contas `GoogleDrive-*` em `~/Library/CloudStorage` e volumes compatíveis em `/Volumes`.
3. O primeiro local compatível é configurado automaticamente, sem configuração adicional.
4. No macOS, quando a raiz contém `Meu Drive` ou `My Drive`, essa área gravável é usada automaticamente para criar os atendimentos.
5. O aplicativo valida existência, acesso, permissão de escrita e espaço disponível.

As configurações são armazenadas pelo plugin Store no diretório de dados do aplicativo definido pelo sistema operacional. Nenhuma conta ou credencial do Google é armazenada.

## Comportamento das transferências

### Preservação de estrutura

Ao selecionar uma pasta, o nome da pasta e sua estrutura interna são preservados:

```text
Origem:
C:\Backup\Empresa

Destino:
E:\Google Drive\[1234567890]\Empresa
```

### Arquivos duplicados

O comportamento padrão é **renomear automaticamente**:

```text
backup.zip
backup (1).zip
backup (2).zip
```

Também é possível:

- substituir o arquivo existente;
- ignorar o novo arquivo;
- perguntar em cada ocorrência.

### Escrita temporária

Cada arquivo é copiado inicialmente com um sufixo temporário:

```text
arquivo.zip.<id-da-operacao>.uploading
```

Depois que a gravação termina corretamente, o arquivo temporário é renomeado para o nome final. Em caso de erro ou cancelamento, o PackDrive tenta remover o conteúdo parcial.

### Cancelamento

O cancelamento:

- interrompe a operação depois do bloco atual;
- preserva arquivos já concluídos;
- não remove arquivos preexistentes;
- remove arquivos temporários da operação quando possível;
- registra a operação como cancelada no histórico.

## Estrutura do projeto

```text
PackDrive/
├── src/
│   ├── App.tsx                 # Interface e fluxos principais
│   ├── App.css                 # Tema e componentes visuais
│   ├── storage.ts              # Persistência de configurações e histórico
│   ├── types.ts                # Contratos TypeScript
│   └── main.tsx                # Entrada do React
├── src-tauri/
│   ├── src/
│   │   ├── lib.rs              # Detecção, navegação, validação e cópia
│   │   └── main.rs             # Entrada nativa
│   ├── capabilities/
│   │   └── default.json        # Permissões dos plugins
│   ├── icons/                  # Ícones dos pacotes nativos
│   ├── Cargo.toml              # Dependências Rust
│   └── tauri.conf.json         # Configuração do aplicativo
├── package.json
├── vite.config.ts
└── README.md
```

## Comandos disponíveis

| Comando | Descrição |
| --- | --- |
| `npm run dev` | Inicia somente o servidor Vite |
| `npm run build` | Verifica o TypeScript e gera o frontend de produção |
| `npm run preview` | Serve localmente o frontend gerado |
| `npm run tauri dev` | Executa o aplicativo Tauri em desenvolvimento |
| `npm run tauri build` | Compila e empacota o aplicativo nativo |
| `cargo test --manifest-path src-tauri/Cargo.toml` | Executa os testes do backend |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` | Executa a análise estática do Rust |

## Validação e testes

Valide o frontend:

```bash
npm run build
```

Formate, teste e analise o backend:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Os testes atuais cobrem nomes de pastas compatíveis com o Windows, o layout usado pelo Google Drive no macOS e o renomeio automático de arquivos duplicados.

## Gerando o instalador

Os pacotes devem ser gerados no sistema operacional de destino com os requisitos do Tauri instalados:

```bash
npm run tauri build
```

Os artefatos são produzidos dentro de:

```text
src-tauri/target/release/bundle/
```

No Windows, o Tauri gera os formatos habilitados, como `.msi` e `.exe`. No macOS, gera o aplicativo `.app` e, conforme a configuração e as ferramentas disponíveis, um `.dmg`.

Para distribuir o aplicativo fora da máquina de desenvolvimento no macOS, configure assinatura de código e notarização da Apple. Builds locais podem ser executados sem essas credenciais.

## Privacidade e segurança

- O PackDrive não acessa a API do Google Drive.
- O aplicativo não solicita login ou token do Google.
- Arquivos não são enviados para um servidor intermediário.
- Configurações e histórico permanecem no computador.
- Origens são lidas em streaming e os dados são gravados diretamente no destino.
- Caminhos de destino são normalizados e precisam permanecer dentro do Drive configurado.
- Nomes com path traversal ou caracteres incompatíveis com o Windows são bloqueados para preservar a portabilidade dos arquivos.
- Antes da cópia são verificados destino, permissão de escrita, origens e espaço disponível.

## Solução de problemas

### O Google Drive não foi encontrado

- Confirme que o Google Drive para computador está aberto e conectado.
- No Windows, verifique se a unidade aparece no Explorador de Arquivos.
- No macOS, verifique se a conta aparece no Finder ou em `~/Library/CloudStorage`.
- Abra **Configurações** e use **Alterar localização**.
- Selecione a pasta raiz do Google Drive montado no computador.

### O Google Drive foi localizado, mas não está pronto

- Confirme que o local detectado continua montado.
- O usuário atual precisa ter permissão de escrita em `Meu Drive`, `My Drive` ou na própria raiz detectada.
- Abra **Configurações** e execute a validação novamente.

### Um arquivo não foi copiado

- Verifique se a origem ainda existe.
- Confirme que o arquivo não está bloqueado por outro programa.
- Verifique o espaço livre no destino.
- Confira no histórico se a operação terminou com erros.
- No Windows, caminhos excessivamente longos podem ser recusados para evitar falhas do sistema.

### O aplicativo não compila no Windows

- Confirme a instalação do Rust com `rustc --version`.
- Confirme a instalação do Node.js com `node --version`.
- Instale o WebView2 e o Microsoft C++ Build Tools.
- Execute `npm install` novamente.
- Consulte os [pré-requisitos oficiais do Tauri](https://v2.tauri.app/start/prerequisites/).

### O aplicativo não compila no macOS

- Confirme a instalação do Rust com `rustc --version`.
- Confirme a instalação do Node.js com `node --version`.
- Execute `xcode-select --install` e aceite a licença do Xcode, se solicitada.
- Execute `npm install` novamente.
- Consulte os [pré-requisitos oficiais do Tauri](https://v2.tauri.app/start/prerequisites/).

### O macOS não permite acessar uma pasta

- Selecione novamente a pasta pelo botão **Alterar localização**.
- Abra **Ajustes do Sistema > Privacidade e Segurança** e confira as permissões de **Arquivos e Pastas** do PackDrive.
- Confirme que o Google Drive para computador está ativo e que a pasta está disponível localmente.

## Limitações conhecidas

- A localização do Drive pode exigir seleção manual em instalações com nomes ou montagens incomuns.
- O PackDrive confirma apenas a cópia para a pasta local; ele não acompanha a conclusão da sincronização com a nuvem.
- O histórico é local ao computador e não é sincronizado entre instalações.
- O aplicativo não resolve conflitos criados posteriormente pelo serviço de sincronização do Google Drive.
