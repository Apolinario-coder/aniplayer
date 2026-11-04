# Instalador do Ani Player

```text
Janela nativa
├─ Arte da estação + degradê
├─ Nome e propósito do aplicativo
├─ Painel da etapa
│  ├─ Título e mensagem de estado
│  ├─ Pasta de instalação e opção de atalho, ou barra de progresso
│  └─ Ação secundária + ação primária
└─ Versão e plataforma
```

| Etapa | Ação primária | Ação secundária |
| --- | --- | --- |
| Inicial | Instalar | Cancelar |
| Em andamento | Instalando… | Cancelar |
| Concluída | Abrir Ani Player | Fechar |
| Erro | Tentar novamente | Fechar |

| Tamanho | Janela em pontos lógicos | Ações |
| --- | --- | --- |
| Padrão | 790 × 650 | Altura 48 |
| Compacto | Mínimo 420 × 450, rolagem vertical | Altura 48 |

| Estado | Aparência e comportamento |
| --- | --- |
| Padrão | Ação primária em `SAKURA` com texto `ON_SAKURA`. Ação secundária em `CONTROL` com texto `INK`. |
| Hover | Superfície secundária em `HOVER`. A ação primária usa o realce do tema do egui. |
| Foco | Contorno externo de 2 pontos em `SAKURA`. |
| Pressionado | Borda ativa do tema em `SAKURA`. |
| Desabilitado | As duas ações ficam desabilitadas durante a cópia. |
| Carregando | Barra de 16 pontos em `SAKURA`, percentual e descrição da etapa. |
| Erro | Mensagem em `ERROR` com ação para tentar novamente. |
| Selecionado | Marca de seleção na opção de atalho. |

O adaptador nativo do protocolo de componentes usa `theme.rs` para superfícies `PAGE` e `PANEL`, texto secundário `MUTED`, borda `BORDER`, raio `CORNER` e espaçamento `GAP`. A altura das ações corresponde a `control.lg` do kit. O tema escuro acompanha o Ani Player.

Botões e caixa de seleção usam os controles do egui e seus papéis no AccessKit. Tab percorre as ações, Espaço alterna a caixa e Enter ou Espaço ativa o botão com foco. Cada etapa inclui um título legível sem depender de cor ou animação. A leitura das mudanças de estado pelo Narrador requer teste nativo.

O [harness HTML](states.html) espelha os estados para os gates web do kit. Capturas e testes do instalador executam separadamente no adaptador egui.

**Alterar…** abre o seletor de pastas do Windows. O campo mostra o destino final, incluindo a subpasta `Ani Player`. A barra reserva 0–10% para preparação, 10–89% para extração medida em bytes e 90–99% para configuração. A conclusão é exibida após o retorno bem-sucedido do instalador.
