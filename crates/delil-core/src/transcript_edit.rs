//! Transkript düzenleme işlemleri ve geri alma (şartname 10.1, 10.3).
//!
//! Bilirkişi kelimeyi/cümleyi düzeltebilir, zaman kodunu değiştirebilir,
//! anlaşılamayan bölümü işaretleyebilir, konuşmacı etiketi ve not ekleyebilir
//! ve değişikliği geri alabilir. Her düzenleme yeni bir **sürüm** üretir; ilk
//! makine çıktısı hiçbir zaman kaybedilmez.

use crate::model::{Transcript, TranscriptSegment};
use crate::status::{QualityFlag, TranscriptStatus};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Tek bir düzenleme işlemi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "op")]
pub enum EditOperation {
    /// Bir segmentin metnini düzeltir.
    SetText { index: usize, text: String },
    /// Bir segmentin zaman kodunu düzeltir.
    SetTiming {
        index: usize,
        start_seconds: f64,
        end_seconds: f64,
    },
    /// Anlaşılamayan bölüm işaretini değiştirir.
    SetUnintelligible { index: usize, marked: bool },
    /// Konuşmacı etiketi ekler/değiştirir.
    SetSpeaker { index: usize, speaker: Option<String> },
    /// Not ekler/değiştirir.
    SetNote { index: usize, note: Option<String> },
    /// Segment kalite bayrağını değiştirir.
    SetQuality { index: usize, quality: QualityFlag },
}

/// Bir düzenlemenin geri alınabilmesi için tutulan kayıt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditRecord {
    pub at: DateTime<Utc>,
    pub actor: String,
    pub operation: EditOperation,
    /// İşlem öncesi segment durumu (geri alma için).
    pub before: TranscriptSegment,
}

/// Düzenleme oturumu: geçerli segmentler ve geri alma yığını.
#[derive(Debug, Clone)]
pub struct TranscriptEditor {
    pub transcript_id: crate::ids::TranscriptId,
    /// Düzenlenmekte olan segmentler (geçerli sürümün kopyası).
    pub segments: Vec<TranscriptSegment>,
    /// Geri alma yığını.
    pub history: Vec<EditRecord>,
    /// Yeni geri alma yığını (geri alındıktan sonra).
    pub redo: Vec<EditRecord>,
    /// Düzenleyen kullanıcı.
    pub actor: String,
}

impl TranscriptEditor {
    /// Bir transkriptin geçerli sürümünden düzenleyici oluşturur.
    pub fn from_transcript(t: &Transcript, actor: impl Into<String>) -> Self {
        Self {
            transcript_id: t.id.clone(),
            segments: t.current().segments.clone(),
            history: Vec::new(),
            redo: Vec::new(),
            actor: actor.into(),
        }
    }

    /// Bir düzenleme uygular.
    pub fn apply(&mut self, op: EditOperation) -> Result<(), EditError> {
        let idx = op.index();
        let before = self
            .segments
            .get(idx)
            .cloned()
            .ok_or(EditError::IndexOutOfRange(idx))?;
        {
            let seg = self
                .segments
                .get_mut(idx)
                .ok_or(EditError::IndexOutOfRange(idx))?;
            match &op {
                EditOperation::SetText { text, .. } => seg.text = text.clone(),
                EditOperation::SetTiming {
                    start_seconds,
                    end_seconds,
                    ..
                } => {
                    seg.start_seconds = *start_seconds;
                    seg.end_seconds = *end_seconds;
                }
                EditOperation::SetUnintelligible { marked, .. } => {
                    seg.marked_unintelligible = *marked;
                    if *marked && seg.quality.severity() < QualityFlag::Unintelligible.severity() {
                        seg.quality = QualityFlag::Unintelligible;
                    }
                }
                EditOperation::SetSpeaker { speaker, .. } => seg.speaker = speaker.clone(),
                EditOperation::SetNote { note, .. } => seg.note = note.clone(),
                EditOperation::SetQuality { quality, .. } => seg.quality = *quality,
            }
        }
        self.history.push(EditRecord {
            at: Utc::now(),
            actor: self.actor.clone(),
            operation: op,
            before,
        });
        self.redo.clear();
        Ok(())
    }

    /// Son düzenlemeyi geri alır.
    pub fn undo(&mut self) -> Option<EditOperation> {
        let rec = self.history.pop()?;
        let idx = rec.operation.index();
        if let Some(seg) = self.segments.get_mut(idx) {
            *seg = rec.before.clone();
        }
        let op = rec.operation.clone();
        self.redo.push(rec);
        Some(op)
    }

    /// Geri alınan son düzenlemeyi yeniden uygular.
    pub fn redo(&mut self) -> Option<EditOperation> {
        let rec = self.redo.pop()?;
        let idx = rec.operation.index();
        if let Some(seg) = self.segments.get_mut(idx) {
            match &rec.operation {
                EditOperation::SetText { text, .. } => seg.text = text.clone(),
                EditOperation::SetTiming {
                    start_seconds,
                    end_seconds,
                    ..
                } => {
                    seg.start_seconds = *start_seconds;
                    seg.end_seconds = *end_seconds;
                }
                EditOperation::SetUnintelligible { marked, .. } => {
                    seg.marked_unintelligible = *marked;
                }
                EditOperation::SetSpeaker { speaker, .. } => seg.speaker = speaker.clone(),
                EditOperation::SetNote { note, .. } => seg.note = note.clone(),
                EditOperation::SetQuality { quality, .. } => seg.quality = *quality,
            }
        }
        let op = rec.operation.clone();
        self.history.push(rec);
        Some(op)
    }

    /// Değişiklik var mı?
    pub fn has_changes(&self) -> bool {
        !self.history.is_empty()
    }

    /// Geçerli düzenlemeleri transkripte yeni bir sürüm olarak uygular.
    ///
    /// İlk makine çıktısı korunur. Sürüm durumu bilirkişi düzeltmesi olarak
    /// işaretlenir.
    pub fn commit_to(&self, t: &mut Transcript) -> u32 {
        let note = format!("{} düzenleme uygulandı", self.history.len());
        t.add_version(
            self.segments.clone(),
            self.actor.clone(),
            TranscriptStatus::ExpertEdited,
            Some(note),
        )
    }

    /// Bilirkişi onayı olarak işaretler.
    pub fn approve(&self, t: &mut Transcript) -> u32 {
        t.add_version(
            self.segments.clone(),
            self.actor.clone(),
            TranscriptStatus::ExpertApproved,
            Some("Bilirkişi tarafından onaylandı".to_string()),
        )
    }
}

impl EditOperation {
    /// İşlemin hedef segment sırası.
    pub fn index(&self) -> usize {
        match self {
            EditOperation::SetText { index, .. }
            | EditOperation::SetTiming { index, .. }
            | EditOperation::SetUnintelligible { index, .. }
            | EditOperation::SetSpeaker { index, .. }
            | EditOperation::SetNote { index, .. }
            | EditOperation::SetQuality { index, .. } => *index,
        }
    }
}

/// Düzenleme hataları.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EditError {
    #[error("Segment sırası aralık dışında: {0}")]
    IndexOutOfRange(usize),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Transcript;
    use crate::ids::AudioAssetId;

    fn seg(i: usize, text: &str) -> TranscriptSegment {
        TranscriptSegment {
            index: i,
            start_seconds: i as f64,
            end_seconds: i as f64 + 1.0,
            text: text.to_string(),
            machine_text: text.to_string(),
            quality: QualityFlag::Normal,
            issues: vec![],
            marked_unintelligible: false,
            speaker: None,
            note: None,
            word_level_timestamps: false,
        }
    }

    fn transcript() -> Transcript {
        Transcript::new_machine(
            AudioAssetId::new(),
            "test",
            vec![seg(0, "merhaba"), seg(1, "dünya")],
            Some("tr".into()),
        )
    }

    #[test]
    fn edit_text_and_undo() {
        let t = transcript();
        let mut ed = TranscriptEditor::from_transcript(&t, "bilirkişi");
        ed.apply(EditOperation::SetText {
            index: 0,
            text: "Merhaba.".into(),
        })
        .unwrap();
        assert_eq!(ed.segments[0].text, "Merhaba.");
        assert!(ed.has_changes());
        ed.undo();
        assert_eq!(ed.segments[0].text, "merhaba");
    }

    #[test]
    fn redo_reapplies() {
        let t = transcript();
        let mut ed = TranscriptEditor::from_transcript(&t, "bilirkişi");
        ed.apply(EditOperation::SetText {
            index: 0,
            text: "Yeni".into(),
        })
        .unwrap();
        ed.undo();
        ed.redo();
        assert_eq!(ed.segments[0].text, "Yeni");
    }

    #[test]
    fn timing_correction() {
        let t = transcript();
        let mut ed = TranscriptEditor::from_transcript(&t, "bilirkişi");
        ed.apply(EditOperation::SetTiming {
            index: 1,
            start_seconds: 1.5,
            end_seconds: 2.5,
        })
        .unwrap();
        assert_eq!(ed.segments[1].start_seconds, 1.5);
        assert_eq!(ed.segments[1].end_seconds, 2.5);
    }

    #[test]
    fn speaker_and_note() {
        let t = transcript();
        let mut ed = TranscriptEditor::from_transcript(&t, "bilirkişi");
        ed.apply(EditOperation::SetSpeaker {
            index: 0,
            speaker: Some("Konuşmacı 1".into()),
        })
        .unwrap();
        ed.apply(EditOperation::SetNote {
            index: 0,
            note: Some("kontrol edildi".into()),
        })
        .unwrap();
        assert_eq!(ed.segments[0].speaker.as_deref(), Some("Konuşmacı 1"));
        assert_eq!(ed.segments[0].note.as_deref(), Some("kontrol edildi"));
    }

    #[test]
    fn mark_unintelligible_sets_quality() {
        let t = transcript();
        let mut ed = TranscriptEditor::from_transcript(&t, "bilirkişi");
        ed.apply(EditOperation::SetUnintelligible {
            index: 0,
            marked: true,
        })
        .unwrap();
        assert!(ed.segments[0].marked_unintelligible);
        assert_eq!(ed.segments[0].quality, QualityFlag::Unintelligible);
    }

    #[test]
    fn commit_preserves_machine_output() {
        let mut t = transcript();
        let mut ed = TranscriptEditor::from_transcript(&t, "bilirkişi");
        ed.apply(EditOperation::SetText {
            index: 0,
            text: "Merhaba.".into(),
        })
        .unwrap();
        let v = ed.commit_to(&mut t);
        assert_eq!(v, 2);
        // İlk makine çıktısı korundu.
        assert_eq!(t.versions[0].segments[0].machine_text, "merhaba");
        assert_eq!(t.versions[0].status, TranscriptStatus::MachineGenerated);
        assert_eq!(t.current().segments[0].text, "Merhaba.");
        assert_eq!(t.current().status, TranscriptStatus::ExpertEdited);
    }

    #[test]
    fn approve_creates_approved_version() {
        let mut t = transcript();
        let ed = TranscriptEditor::from_transcript(&t, "bilirkişi");
        ed.approve(&mut t);
        assert_eq!(t.current().status, TranscriptStatus::ExpertApproved);
        assert_eq!(t.current().version, 2);
    }

    #[test]
    fn out_of_range_edit_fails() {
        let t = transcript();
        let mut ed = TranscriptEditor::from_transcript(&t, "x");
        assert_eq!(
            ed.apply(EditOperation::SetText {
                index: 99,
                text: "x".into()
            }),
            Err(EditError::IndexOutOfRange(99))
        );
    }
}
