/// Tantivy full-text search index for offline semantic-style search
use anyhow::Result;
use std::path::Path;
use tantivy::{
    collector::TopDocs,
    directory::MmapDirectory,
    doc,
    query::QueryParser,
    schema::{Schema, STORED, TEXT, Value},
    Index, IndexWriter, ReloadPolicy, TantivyDocument,
};

pub struct SearchIndex {
    index: Index,
    schema: Schema,
}

impl SearchIndex {
    pub fn open(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir)?;

        let mut schema_builder = Schema::builder();
        schema_builder.add_text_field("thread_id", STORED);
        schema_builder.add_text_field("subject", TEXT | STORED);
        schema_builder.add_text_field("body", TEXT);
        schema_builder.add_text_field("sender", TEXT | STORED);
        let schema = schema_builder.build();

        let directory = MmapDirectory::open(dir)?;
        let index = Index::open_or_create(directory, schema.clone())?;

        Ok(Self { index, schema })
    }

    pub fn add_document(
        &self,
        thread_id: &str,
        subject: &str,
        body: &str,
        sender: &str,
    ) -> Result<()> {
        let mut writer: IndexWriter = self.index.writer(50_000_000)?;

        let thread_id_field = self.schema.get_field("thread_id").unwrap();
        let subject_field = self.schema.get_field("subject").unwrap();
        let body_field = self.schema.get_field("body").unwrap();
        let sender_field = self.schema.get_field("sender").unwrap();

        writer.add_document(doc!(
            thread_id_field => thread_id,
            subject_field => subject,
            body_field => body,
            sender_field => sender,
        ))?;

        writer.commit()?;
        Ok(())
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<String>> {
        let reader = self
            .index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        let searcher = reader.searcher();

        let subject_field = self.schema.get_field("subject").unwrap();
        let body_field = self.schema.get_field("body").unwrap();
        let sender_field = self.schema.get_field("sender").unwrap();
        let thread_id_field = self.schema.get_field("thread_id").unwrap();

        let query_parser =
            QueryParser::for_index(&self.index, vec![subject_field, body_field, sender_field]);

        let query = query_parser.parse_query(query)?;
        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        let mut thread_ids = Vec::new();
        for (_score, doc_addr) in top_docs {
            let doc: TantivyDocument = searcher.doc(doc_addr)?;
            if let Some(val) = doc.get_first(thread_id_field) {
                if let Some(id) = val.as_str() {
                    thread_ids.push(id.to_string());
                }
            }
        }

        Ok(thread_ids)
    }
}
