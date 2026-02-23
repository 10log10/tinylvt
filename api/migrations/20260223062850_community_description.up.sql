ALTER TABLE communities ADD COLUMN description TEXT;
ALTER TABLE communities ADD COLUMN community_image_id UUID REFERENCES site_images(id);
